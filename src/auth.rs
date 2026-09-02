use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tower_sessions::Session;

use crate::user::{discord_avatar_url, DiscordProfile, UiPrefsUpdate, User, UserStore};

const DISCORD_AUTHORIZE_URL: &str = "https://discord.com/api/oauth2/authorize";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
const DISCORD_USER_URL: &str = "https://discord.com/api/users/@me";
const DISCORD_USER_GUILDS_URL: &str = "https://discord.com/api/users/@me/guilds";
const DISCORD_OAUTH_SCOPES: &str = "identify guilds";
const DISCORD_GUILDS_PAGE_SIZE: usize = 200;
const DEFAULT_DISCORD_GUILD_ID: &str = "299262973241720832";

pub const SESSION_USER_ID: &str = "user_id";
pub const SESSION_SECONDARY_VIEW_MODE: &str = "secondary_view_mode";
pub const SESSION_SCENARIO_SLUG: &str = "scenario_slug";
pub const SESSION_ARMY_SORT_MODE: &str = "army_sort_mode";
pub const SESSION_PLAYER_SORT_MODE: &str = "player_sort_mode";
pub const SESSION_TOURNAMENT_COMPLETED_VIEW_MODE: &str = "tournament_completed_view_mode";
pub const DEFAULT_SECONDARY_VIEW_MODE: &str = "liste";
pub const DEFAULT_ARMY_SORT_MODE: &str = "win_rate";
pub const DEFAULT_PLAYER_SORT_MODE: &str = "elo";
pub const DEFAULT_TOURNAMENT_COMPLETED_VIEW_MODE: &str = "compressed";

pub fn parse_secondary_view_mode(value: &str) -> Option<&'static str> {
    match value {
        "liste" => Some("liste"),
        "cartes" => Some("cartes"),
        _ => None,
    }
}

pub fn parse_army_sort_mode(value: &str) -> Option<&'static str> {
    match value {
        "win_rate" => Some("win_rate"),
        "matches" => Some("matches"),
        _ => None,
    }
}

pub fn parse_player_sort_mode(value: &str) -> Option<&'static str> {
    match value {
        "elo" => Some("elo"),
        "win_rate" => Some("win_rate"),
        "matches" => Some("matches"),
        _ => None,
    }
}

pub fn parse_tournament_completed_view_mode(value: &str) -> Option<&'static str> {
    match value {
        "detailed" => Some("detailed"),
        "compressed" => Some("compressed"),
        _ => None,
    }
}

pub fn normalize_scenario_slug(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub frontend_url: String,
    pub required_guild_id: String,
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
            required_guild_id: std::env::var("DISCORD_GUILD_ID")
                .unwrap_or_else(|_| DEFAULT_DISCORD_GUILD_ID.into()),
        })
    }

    pub fn authorize_url(&self) -> String {
        format!(
            "{DISCORD_AUTHORIZE_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}",
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(DISCORD_OAUTH_SCOPES),
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

    sync_pref_on_login(
        users,
        session,
        user.id,
        user.secondary_view_mode.as_deref(),
        SESSION_SECONDARY_VIEW_MODE,
        parse_secondary_view_mode,
        |value| UiPrefsUpdate {
            secondary_view_mode: Some(value.to_string()),
            ..UiPrefsUpdate::default()
        },
    )
    .await?;

    sync_string_pref_on_login(
        users,
        session,
        user.id,
        user.scenario_slug.as_deref(),
        SESSION_SCENARIO_SLUG,
        |value| UiPrefsUpdate {
            scenario_slug: Some(value.to_string()),
            ..UiPrefsUpdate::default()
        },
    )
    .await?;

    sync_pref_on_login(
        users,
        session,
        user.id,
        user.army_sort_mode.as_deref(),
        SESSION_ARMY_SORT_MODE,
        parse_army_sort_mode,
        |value| UiPrefsUpdate {
            army_sort_mode: Some(value.to_string()),
            ..UiPrefsUpdate::default()
        },
    )
    .await?;

    sync_pref_on_login(
        users,
        session,
        user.id,
        user.player_sort_mode.as_deref(),
        SESSION_PLAYER_SORT_MODE,
        parse_player_sort_mode,
        |value| UiPrefsUpdate {
            player_sort_mode: Some(value.to_string()),
            ..UiPrefsUpdate::default()
        },
    )
    .await?;

    sync_pref_on_login(
        users,
        session,
        user.id,
        user.tournament_completed_view_mode.as_deref(),
        SESSION_TOURNAMENT_COMPLETED_VIEW_MODE,
        parse_tournament_completed_view_mode,
        |value| UiPrefsUpdate {
            tournament_completed_view_mode: Some(value.to_string()),
            ..UiPrefsUpdate::default()
        },
    )
    .await?;

    Ok(user)
}

async fn sync_pref_on_login(
    users: &UserStore,
    session: &Session,
    user_id: i64,
    user_value: Option<&str>,
    session_key: &str,
    parse: fn(&str) -> Option<&'static str>,
    to_update: fn(&str) -> UiPrefsUpdate,
) -> Result<()> {
    if let Some(mode) = user_value.and_then(parse) {
        session
            .insert(session_key, mode.to_string())
            .await
            .context("impossible d'enregistrer la préférence")?;
    } else {
        let from_session: Option<String> = session
            .get(session_key)
            .await
            .context("impossible de lire la préférence")?;
        if let Some(mode) = from_session.as_deref().and_then(parse) {
            users
                .update_ui_prefs(user_id, to_update(mode))
                .context("impossible de sauvegarder la préférence")?;
        }
    }
    Ok(())
}

async fn sync_string_pref_on_login(
    users: &UserStore,
    session: &Session,
    user_id: i64,
    user_value: Option<&str>,
    session_key: &str,
    to_update: fn(&str) -> UiPrefsUpdate,
) -> Result<()> {
    if let Some(value) = user_value.and_then(normalize_scenario_slug) {
        session
            .insert(session_key, value)
            .await
            .context("impossible d'enregistrer la préférence")?;
    } else {
        let from_session: Option<String> = session
            .get(session_key)
            .await
            .context("impossible de lire la préférence")?;
        if let Some(value) = from_session.as_deref().and_then(normalize_scenario_slug) {
            users
                .update_ui_prefs(user_id, to_update(&value))
                .context("impossible de sauvegarder la préférence")?;
        }
    }
    Ok(())
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

#[derive(Debug, Deserialize)]
struct DiscordGuildResponse {
    id: String,
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
    let access_token = token_response.access_token;

    ensure_required_guild_membership(&client, &access_token, &auth.required_guild_id).await?;

    let discord_user = client
        .get(DISCORD_USER_URL)
        .bearer_auth(&access_token)
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

async fn ensure_required_guild_membership(
    client: &Client,
    access_token: &str,
    required_guild_id: &str,
) -> Result<()> {
    let mut after: Option<String> = None;

    loop {
        let mut request = client
            .get(DISCORD_USER_GUILDS_URL)
            .bearer_auth(access_token)
            .query(&[("limit", DISCORD_GUILDS_PAGE_SIZE.to_string())]);
        if let Some(cursor) = after.as_deref() {
            request = request.query(&[("after", cursor)]);
        }

        let response = request
            .send()
            .await
            .context("échec de la requête serveurs Discord")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("réponse serveurs Discord illisible")?;

        if !status.is_success() {
            anyhow::bail!("liste des serveurs Discord refusée (HTTP {status}) : {body}");
        }

        let guilds = serde_json::from_str::<Vec<DiscordGuildResponse>>(&body)
            .context("réponse serveurs Discord invalide")?;

        if guilds.iter().any(|guild| guild.id == required_guild_id) {
            return Ok(());
        }

        if guilds.len() < DISCORD_GUILDS_PAGE_SIZE {
            anyhow::bail!(
                "connexion refusée : vous devez être membre du serveur Discord Poissonnerie"
            );
        }

        let Some(last) = guilds.last() else {
            anyhow::bail!(
                "connexion refusée : vous devez être membre du serveur Discord Poissonnerie"
            );
        };
        after = Some(last.id.clone());
    }
}
