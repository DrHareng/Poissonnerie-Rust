use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use poissonnerie_elo::{
    Leaderboard, MatchOutcome, MatchScores, DEFAULT_K_FACTOR, DEFAULT_RATING,
};

#[derive(Parser)]
#[command(
    name = "poissonnerie-elo",
    about = "Classement ELO pour la Poissonnerie",
    version
)]
struct Cli {
    /// Base SQLite (joueurs, matchs et armées)
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,

    /// Facteur K du système ELO
    #[arg(long, default_value_t = DEFAULT_K_FACTOR)]
    k: f64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ajouter un nouveau joueur
    Add {
        /// Nom du joueur
        name: String,
    },

    /// Enregistrer le résultat d'un match
    Match {
        /// Premier joueur
        player1: String,

        /// Deuxième joueur
        player2: String,

        /// Résultat : victoire du joueur 1, du joueur 2, ou nul
        #[arg(value_parser = parse_outcome)]
        outcome: MatchOutcome,
    },

    /// Afficher le classement
    Ranking,

    /// Afficher les détails d'un joueur
    Show {
        /// Nom du joueur
        name: String,
    },

    /// Lister tous les joueurs
    List,
}

fn parse_outcome(value: &str) -> Result<MatchOutcome, String> {
    match value.to_lowercase().as_str() {
        "1" | "j1" | "win1" | "victoire1" | "v1" => Ok(MatchOutcome::Player1Win),
        "2" | "j2" | "win2" | "victoire2" | "v2" => Ok(MatchOutcome::Player2Win),
        "n" | "nul" | "draw" | "x" => Ok(MatchOutcome::Draw),
        _ => Err(
            "résultat invalide : utilisez 1, 2, nul (ou v1, v2, draw)".to_string(),
        ),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut board = Leaderboard::load(&cli.db)?;

    match cli.command {
        Commands::Add { name } => {
            board.add_player(&name)?;
            board.save(&cli.db)?;
            println!(
                "Joueur « {} » ajouté avec un classement initial de {:.0}.",
                name, DEFAULT_RATING
            );
        }

        Commands::Match {
            player1,
            player2,
            outcome,
        } => {
            let record = board.record_match(
                &player1,
                &player2,
                outcome,
                cli.k,
                MatchScores::default(),
                None,
                None,
                None,
                None,
                None,
            )?;
            board.save(&cli.db)?;

            println!("Match enregistré :");
            println!(
                "  {} : {:.0} → {:.0}",
                player1, record.player1_old, record.player1_new
            );
            println!(
                "  {} : {:.0} → {:.0}",
                player2, record.player2_old, record.player2_new
            );
        }

        Commands::Ranking => {
            let ranking = board.ranking();
            if ranking.is_empty() {
                println!("Aucun joueur enregistré.");
                return Ok(());
            }

            println!("Classement ELO");
            println!("{:-<60}", "");
            println!(
                "{:>4}  {:<20} {:>7}  {:>5} {:>5} {:>5}",
                "#", "Joueur", "ELO", "V", "N", "D"
            );
            println!("{:-<60}", "");

            for (index, player) in ranking.iter().enumerate() {
                println!(
                    "{:>4}  {:<20} {:>7.0}  {:>5} {:>5} {:>5}",
                    index + 1,
                    player.name,
                    player.rating,
                    player.wins,
                    player.draws,
                    player.losses
                );
            }
        }

        Commands::Show { name } => {
            let player = board.get_player(&name)?;
            println!("Joueur : {}", player.name);
            println!("ELO    : {:.0}", player.rating);
            println!(
                "Matchs : {} ({}V / {}N / {}D)",
                player.matches_played(),
                player.wins,
                player.draws,
                player.losses
            );
        }

        Commands::List => {
            let ranking = board.ranking();
            if ranking.is_empty() {
                println!("Aucun joueur enregistré.");
                return Ok(());
            }

            for player in ranking {
                println!("{} — {:.0} ELO", player.name, player.rating);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_outcome_accepts_french_aliases() {
        assert_eq!(parse_outcome("v1").unwrap(), MatchOutcome::Player1Win);
        assert_eq!(parse_outcome("nul").unwrap(), MatchOutcome::Draw);
        assert!(parse_outcome("foo").is_err());
    }
}
