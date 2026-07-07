use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tower_sessions::Session;

use crate::user::{discord_avatar_url, DiscordProfile, User, UserStore};

const DISCORD_AUTHORIZE_URL: &str = "https://discord.com/api/oauth2/authorize";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const DISCORD_USER_URL: &str = "https://discord.com/api/users/@me";

pub const SESSION_USER_ID: &str = "user_id";

#[derive(Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub frontend_url: String,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            client_id: std::env::var("DISCORD_CLIENT_ID")
                .context("DISCORD_CLIENT_ID manquant")?,
            client_secret: std::env::var("DISCORD_CLIENT_SECRET")
                .context("DISCORD_CLIENT_SECRET manquant")?,
            redirect_uri: std::env::var("DISCORD_REDIRECT_URI")
                .unwrap_or_else(|_| "http://127.0.0.1:3000/api/auth/callback".into()),
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:5173".into()),
        })
    }

    pub fn authorize_url(&self) -> String {
        format!(
            "{DISCORD_AUTHORIZE_URL}?client_id={}&redirect_uri={}&response_type=code&scope=identify",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
        )
    }
}

pub async fn login_with_code(
    auth: &AuthConfig,
    users: &UserStore,
    session: &Session,
    code: &str,
) -> Result<User> {
    let profile = exchange_code(auth, code).await?;
    let user = users.upsert_from_discord(&profile)?;
    session
        .insert(SESSION_USER_ID, user.id)
        .await
        .context("impossible d'enregistrer la session")?;
    Ok(user)
}

pub async fn current_user(users: &UserStore, session: &Session) -> Result<Option<User>> {
    let user_id: Option<i64> = session
        .get(SESSION_USER_ID)
        .await
        .context("impossible de lire la session")?;
    let Some(user_id) = user_id else {
        return Ok(None);
    };
    users.get_by_id(user_id)
}

pub async fn logout(session: &Session) -> Result<()> {
    session.flush().await.context("impossible de fermer la session")
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}

#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUserResponse {
    id: String,
    username: String,
    #[serde(default)]
    global_name: Option<String>,
    avatar: Option<String>,
}

async fn exchange_code(auth: &AuthConfig, code: &str) -> Result<DiscordProfile> {
    let client = Client::new();
    let response = client
        .post(DISCORD_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&client_secret={}&grant_type=authorization_code&code={}&redirect_uri={}",
            urlencoding::encode(&auth.client_id),
            urlencoding::encode(&auth.client_secret),
            urlencoding::encode(code),
            urlencoding::encode(&auth.redirect_uri),
        ))
        .send()
        .await
        .context("échec de la requête token Discord")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("réponse token Discord illisible")?;

    if !status.is_success() {
        anyhow::bail!("token Discord refusé (HTTP {status}) : {body}");
    }

    let token_response = serde_json::from_str::<DiscordTokenResponse>(&body)
        .context("réponse token Discord invalide")?;

    let discord_user = client
        .get(DISCORD_USER_URL)
        .bearer_auth(token_response.access_token)
        .send()
        .await
        .context("échec de la requête profil Discord")?
        .error_for_status()
        .context("profil Discord refusé")?
        .json::<DiscordUserResponse>()
        .await
        .context("réponse profil Discord invalide")?;

    let display_name = discord_user
        .global_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| discord_user.username.clone());

    Ok(DiscordProfile {
        discord_id: discord_user.id.clone(),
        username: discord_user.username,
        display_name,
        avatar_url: discord_avatar_url(&discord_user.id, discord_user.avatar.as_deref()),
    })
}
