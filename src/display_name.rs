use std::collections::HashMap;

use crate::{Leaderboard, MatchRecord, Player, UserStore};

pub struct PlayerDisplayResolver<'a> {
    board: &'a Leaderboard,
    users: &'a UserStore,
    cache: HashMap<String, String>,
}

impl<'a> PlayerDisplayResolver<'a> {
    pub fn new(board: &'a Leaderboard, users: &'a UserStore) -> Self {
        let mut cache = HashMap::new();
        for player in board.ranking() {
            if let Ok(display_name) = Self::resolve_uncached(board, users, &player.name) {
                cache.insert(player.name.clone(), display_name);
            }
        }

        Self {
            board,
            users,
            cache,
        }
    }

    pub fn resolve(&self, player_name: &str) -> String {
        if let Some(display_name) = self.cache.get(player_name) {
            return display_name.clone();
        }

        Self::resolve_uncached(self.board, self.users, player_name)
            .unwrap_or_else(|_| player_name.to_string())
    }

    pub fn resolve_player(&self, player: &Player) -> String {
        self.resolve(&player.name)
    }

    fn resolve_uncached(
        board: &Leaderboard,
        users: &UserStore,
        player_name: &str,
    ) -> anyhow::Result<String> {
        let player = board.get_player(player_name)?;
        Ok(Self::resolve_from_player(users, player))
    }

    fn resolve_from_player(users: &UserStore, player: &Player) -> String {
        if let Some(discord_username) = player.discord_username.as_deref() {
            if let Ok(Some(user)) = users.get_by_username(discord_username) {
                return user.effective_display_name().to_string();
            }
        }
        player.name.clone()
    }

    pub fn enrich_match(&self, record: MatchRecord) -> EnrichedMatchRecord {
        EnrichedMatchRecord {
            player1_display_name: self.resolve(&record.player1),
            player2_display_name: self.resolve(&record.player2),
            record,
        }
    }

    pub fn enrich_tournament_detail(
        &self,
        mut detail: crate::tournament::TournamentDetail,
    ) -> crate::tournament::TournamentDetail {
        for registration in &mut detail.registrations {
            registration.player_display_name = Some(self.resolve(&registration.player_name));
        }

        for pool in &mut detail.pools {
            for player in &mut pool.players {
                player.player_display_name = Some(self.resolve(&player.player_name));
            }
        }

        for tournament_match in &mut detail.matches {
            if let Some(player1) = &tournament_match.player1 {
                tournament_match.player1_display_name = Some(self.resolve(player1));
            }
            if let Some(player2) = &tournament_match.player2 {
                tournament_match.player2_display_name = Some(self.resolve(player2));
            }
            if let Some(forfeit_player) = &tournament_match.forfeit_player {
                tournament_match.forfeit_player_display_name = Some(self.resolve(forfeit_player));
            }
        }

        detail
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedMatchRecord {
    #[serde(flatten)]
    pub record: MatchRecord,
    pub player1_display_name: String,
    pub player2_display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user::{DiscordProfile, UserStore};

    #[test]
    fn resolver_uses_linked_user_display_name() {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-display-name-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let mut board = Leaderboard::default();
        board
            .add_player_for_discord_username("Dr Hareng", "drhareng")
            .unwrap();

        let users = UserStore::open(&path).unwrap();
        users
            .upsert_from_discord(&DiscordProfile {
                discord_id: "1".into(),
                username: "drhareng".into(),
                display_name: "Dr Hareng Discord".into(),
                avatar_url: "https://example.test/a.png".into(),
            })
            .unwrap();
        users
            .update_local_profile(
                1,
                crate::user::LocalProfileUpdate {
                    local_display_name: Some(Some("Capitaine Hareng".into())),
                    local_avatar_url: None,
                },
            )
            .unwrap();

        let resolver = PlayerDisplayResolver::new(&board, &users);
        assert_eq!(resolver.resolve("Dr Hareng"), "Capitaine Hareng");

        let _ = std::fs::remove_file(path);
    }
}
