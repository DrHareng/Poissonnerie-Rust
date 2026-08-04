use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;
use tower_sessions::{
    session::{Id, Record},
    session_store::{self, SessionStore},
};

use crate::migrate::migrate;

#[derive(Clone, Debug)]
pub struct SqliteSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("impossible de créer le dossier {}", parent.display())
            })?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir {}", path.display()))?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> session_store::Result<T>) -> session_store::Result<T> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| session_store::Error::Backend("mutex sessions empoisonné".into()))?;
        f(&conn)
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        self.with_conn(|conn| {
            loop {
                let id = record.id.to_string();
                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM sessions WHERE id = ?1",
                        params![id],
                        |_| Ok(true),
                    )
                    .optional()
                    .map_err(|error| session_store::Error::Backend(error.to_string()))?
                    .unwrap_or(false);
                if !exists {
                    break;
                }
                record.id = Id::default();
            }
            upsert_session(conn, record)
        })
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.with_conn(|conn| upsert_session(conn, record))
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let id = session_id.to_string();
        self.with_conn(|conn| {
            let row = conn
                .query_row(
                    "SELECT id, data, expiry_date FROM sessions WHERE id = ?1",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| session_store::Error::Backend(error.to_string()))?;

            let Some((id, data, expiry_date)) = row else {
                return Ok(None);
            };

            let expiry_date = OffsetDateTime::from_unix_timestamp(expiry_date)
                .map_err(|error| session_store::Error::Decode(error.to_string()))?;
            if expiry_date <= OffsetDateTime::now_utc() {
                conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])
                    .map_err(|error| session_store::Error::Backend(error.to_string()))?;
                return Ok(None);
            }

            let data = serde_json::from_slice(&data)
                .map_err(|error| session_store::Error::Decode(error.to_string()))?;
            let id = Id::from_str(&id)
                .map_err(|error| session_store::Error::Decode(error.to_string()))?;
            Ok(Some(Record {
                id,
                data,
                expiry_date,
            }))
        })
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let id = session_id.to_string();
        self.with_conn(|conn| {
            conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])
                .map_err(|error| session_store::Error::Backend(error.to_string()))?;
            Ok(())
        })
    }
}

fn upsert_session(conn: &Connection, record: &Record) -> session_store::Result<()> {
    let data = serde_json::to_vec(&record.data)
        .map_err(|error| session_store::Error::Encode(error.to_string()))?;
    conn.execute(
        "INSERT INTO sessions (id, data, expiry_date) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET data = excluded.data, expiry_date = excluded.expiry_date",
        params![
            record.id.to_string(),
            data,
            record.expiry_date.unix_timestamp()
        ],
    )
    .map_err(|error| session_store::Error::Backend(error.to_string()))?;
    Ok(())
}
