use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub discord_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
    pub local_display_name: Option<String>,
    pub local_avatar_url: Option<String>,
    pub is_admin: bool,
    pub created_at: u64,
    pub last_login_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserResponse {
    pub id: i64,
    pub discord_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
    pub effective_display_name: String,
    pub effective_avatar_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_avatar_url: Option<String>,
    pub is_admin: bool,
    pub created_at: u64,
    pub last_login_at: u64,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            effective_display_name: user.effective_display_name().to_string(),
            effective_avatar_url: user.effective_avatar_url().to_string(),
            id: user.id,
            discord_id: user.discord_id,
            username: user.username,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
            local_display_name: user.local_display_name,
            local_avatar_url: user.local_avatar_url,
            is_admin: user.is_admin,
            created_at: user.created_at,
            last_login_at: user.last_login_at,
        }
    }
}

impl User {
    pub fn effective_display_name(&self) -> &str {
        self.local_display_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&self.display_name)
    }

    pub fn effective_avatar_url(&self) -> &str {
        self.local_avatar_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&self.avatar_url)
    }

    pub fn has_local_display_name(&self) -> bool {
        self.local_display_name
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

#[derive(Debug, Clone)]
pub struct DiscordProfile {
    pub discord_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Default)]
pub struct LocalProfileUpdate {
    pub local_display_name: Option<Option<String>>,
    pub local_avatar_url: Option<Option<String>>,
}

pub struct UserStore {
    conn: Mutex<Connection>,
}

impl UserStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("impossible de créer {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir {}", path.display()))?;
        migrate(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn upsert_from_discord(&self, profile: &DiscordProfile) -> Result<User> {
        let now = now_unix();
        let conn = self.conn.lock().unwrap();

        if self.get_by_discord_id_in_conn(&conn, &profile.discord_id)?.is_some() {
            conn.execute(
                "
                UPDATE users
                SET username = ?1,
                    display_name = ?2,
                    avatar_url = ?3,
                    last_login_at = ?4
                WHERE discord_id = ?5
                ",
                params![
                    profile.username,
                    profile.display_name,
                    profile.avatar_url,
                    now,
                    profile.discord_id,
                ],
            )?;
            return self
                .get_by_discord_id_in_conn(&conn, &profile.discord_id)?
                .with_context(|| "utilisateur introuvable après mise à jour");
        }

        conn.execute(
            "
            INSERT INTO users (
                discord_id, username, display_name, avatar_url,
                created_at, last_login_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?5)
            ",
            params![
                profile.discord_id,
                profile.username,
                profile.display_name,
                profile.avatar_url,
                now,
            ],
        )?;

        let id = conn.last_insert_rowid();
        self.get_by_id_in_conn(&conn, id)?
            .with_context(|| "utilisateur introuvable après création")
    }

    pub fn update_local_profile(
        &self,
        user_id: i64,
        update: LocalProfileUpdate,
    ) -> Result<User> {
        let conn = self.conn.lock().unwrap();
        let current = self
            .get_by_id_in_conn(&conn, user_id)?
            .with_context(|| "utilisateur introuvable")?;

        let local_display_name = match update.local_display_name {
            Some(value) => normalize_optional_text(value),
            None => current.local_display_name,
        };
        let local_avatar_url = match update.local_avatar_url {
            Some(value) => normalize_optional_text(value),
            None => current.local_avatar_url,
        };

        conn.execute(
            "
            UPDATE users
            SET local_display_name = ?1,
                local_avatar_url = ?2
            WHERE id = ?3
            ",
            params![local_display_name, local_avatar_url, user_id],
        )?;

        self.get_by_id_in_conn(&conn, user_id)?
            .with_context(|| "utilisateur introuvable après mise à jour")
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        self.get_by_id_in_conn(&conn, id)
    }

    pub fn get_by_discord_id(&self, discord_id: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        self.get_by_discord_id_in_conn(&conn, discord_id)
    }

    pub fn get_by_username(&self, username: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().unwrap();
        self.get_by_username_in_conn(&conn, username)
    }

    fn get_by_id_in_conn(&self, conn: &Connection, id: i64) -> Result<Option<User>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, discord_id, username, display_name, avatar_url,
                   local_display_name, local_avatar_url, is_admin,
                   created_at, last_login_at
            FROM users
            WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_user(row)?));
        }
        Ok(None)
    }

    fn get_by_discord_id_in_conn(
        &self,
        conn: &Connection,
        discord_id: &str,
    ) -> Result<Option<User>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, discord_id, username, display_name, avatar_url,
                   local_display_name, local_avatar_url, is_admin,
                   created_at, last_login_at
            FROM users
            WHERE discord_id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![discord_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_user(row)?));
        }
        Ok(None)
    }

    fn get_by_username_in_conn(
        &self,
        conn: &Connection,
        username: &str,
    ) -> Result<Option<User>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, discord_id, username, display_name, avatar_url,
                   local_display_name, local_avatar_url, is_admin,
                   created_at, last_login_at
            FROM users
            WHERE lower(username) = lower(?1)
            ",
        )?;
        let mut rows = stmt.query(params![username])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_user(row)?));
        }
        Ok(None)
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn row_to_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        discord_id: row.get(1)?,
        username: row.get(2)?,
        display_name: row.get(3)?,
        avatar_url: row.get(4)?,
        local_display_name: row.get(5)?,
        local_avatar_url: row.get(6)?,
        is_admin: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        last_login_at: row.get(9)?,
    })
}

pub fn discord_avatar_url(discord_id: &str, avatar: Option<&str>) -> String {
    match avatar {
        Some(hash) => format!("https://cdn.discordapp.com/avatars/{discord_id}/{hash}.png"),
        None => {
            let index = discord_id.parse::<u64>().unwrap_or(0) % 5;
            format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "poissonnerie-users-test-{}",
            std::process::id()
        ))
    }

    #[test]
    fn upsert_creates_then_updates_user() {
        let path = temp_db_path();
        let _ = std::fs::remove_file(&path);

        let store = UserStore::open(&path).unwrap();
        let profile = DiscordProfile {
            discord_id: "42".into(),
            username: "alice".into(),
            display_name: "Alice".into(),
            avatar_url: discord_avatar_url("42", Some("abc")),
        };

        let created = store.upsert_from_discord(&profile).unwrap();
        assert_eq!(created.username, "alice");

        let updated_profile = DiscordProfile {
            discord_id: "42".into(),
            username: "alice_new".into(),
            display_name: "Alice Updated".into(),
            avatar_url: discord_avatar_url("42", Some("def")),
        };
        let updated = store.upsert_from_discord(&updated_profile).unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.username, "alice_new");
        assert_eq!(updated.display_name, "Alice Updated");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn local_profile_overrides_effective_values() {
        let path = temp_db_path();
        let _ = std::fs::remove_file(&path);

        let store = UserStore::open(&path).unwrap();
        let user = store
            .upsert_from_discord(&DiscordProfile {
                discord_id: "42".into(),
                username: "alice".into(),
                display_name: "Alice".into(),
                avatar_url: "https://discord.test/alice.png".into(),
            })
            .unwrap();

        let updated = store
            .update_local_profile(
                user.id,
                LocalProfileUpdate {
                    local_display_name: Some(Some("Sardine".into())),
                    local_avatar_url: Some(Some("https://example.test/avatar.png".into())),
                },
            )
            .unwrap();

        assert_eq!(updated.effective_display_name(), "Sardine");
        assert_eq!(
            updated.effective_avatar_url(),
            "https://example.test/avatar.png"
        );
        assert!(updated.has_local_display_name());

        let cleared = store
            .update_local_profile(
                user.id,
                LocalProfileUpdate {
                    local_display_name: Some(None),
                    local_avatar_url: None,
                },
            )
            .unwrap();

        assert_eq!(cleared.effective_display_name(), "Alice");
        assert!(!cleared.has_local_display_name());

        let _ = std::fs::remove_file(path);
    }
}
