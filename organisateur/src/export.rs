use rust_xlsxwriter::{Format, Workbook};

use crate::engine::inspect;
use crate::model::Tournament;

pub fn export_xlsx(tournament: &Tournament) -> Result<Vec<u8>, String> {
    let view = inspect(tournament);
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold();

    let standings = workbook.add_worksheet();
    standings
        .set_name("Classement")
        .map_err(|error| error.to_string())?;
    let titles = [
        "Rang",
        "Joueur",
        "Sectorielle",
        "TP",
        "OP",
        "VP",
        "Rondes jouées",
        "OP final",
        "VP final",
        "Victoires",
        "Nuls",
        "Défaites",
        "Byes",
        "Forfait",
    ];
    for (column, title) in titles.iter().enumerate() {
        standings
            .write_string_with_format(0, column as u16, *title, &header)
            .map_err(|error| error.to_string())?;
    }
    for (index, row) in view.standings.iter().enumerate() {
        let line = index as u32 + 1;
        write_number(standings, line, 0, f64::from(row.rank))?;
        write_string(standings, line, 1, &row.name)?;
        write_string(standings, line, 2, &row.sectorial_name)?;
        write_number(standings, line, 3, f64::from(row.tp))?;
        write_number(standings, line, 4, f64::from(row.op))?;
        write_number(standings, line, 5, f64::from(row.vp))?;
        write_number(standings, line, 6, f64::from(row.rounds_played))?;
        write_number(standings, line, 7, row.op_final)?;
        write_number(standings, line, 8, row.vp_final)?;
        write_number(standings, line, 9, f64::from(row.wins))?;
        write_number(standings, line, 10, f64::from(row.draws))?;
        write_number(standings, line, 11, f64::from(row.losses))?;
        write_number(standings, line, 12, f64::from(row.byes))?;
        write_string(standings, line, 13, if row.dropped { "Oui" } else { "" })?;
    }
    standings
        .set_column_width(1, 28)
        .map_err(|error| error.to_string())?;
    standings
        .set_column_width(2, 36)
        .map_err(|error| error.to_string())?;

    let rounds = workbook.add_worksheet();
    rounds
        .set_name("Rondes")
        .map_err(|error| error.to_string())?;
    let round_titles = [
        "Ronde",
        "Table",
        "Joueur 1",
        "Sectorielle 1",
        "OP 1",
        "VP 1",
        "TP 1",
        "Joueur 2",
        "Sectorielle 2",
        "OP 2",
        "VP 2",
        "TP 2",
        "Forfait",
    ];
    for (column, title) in round_titles.iter().enumerate() {
        rounds
            .write_string_with_format(0, column as u16, *title, &header)
            .map_err(|error| error.to_string())?;
    }
    let mut line = 1u32;
    for round in &view.rounds {
        for game in &round.matches {
            write_number(rounds, line, 0, f64::from(round.round))?;
            match game.table {
                Some(table) => write_number(rounds, line, 1, f64::from(table))?,
                None => write_string(rounds, line, 1, "")?,
            }
            write_string(rounds, line, 2, &game.player1.name)?;
            write_string(rounds, line, 3, &game.player1.sectorial_name)?;
            write_optional_u8(rounds, line, 4, game.p1_op)?;
            write_optional_u16(rounds, line, 5, game.p1_vp)?;
            write_optional_u32(rounds, line, 6, game.p1_tp)?;
            if game.bye {
                write_string(rounds, line, 7, "Bye")?;
                write_string(rounds, line, 8, "")?;
            } else if let Some(player2) = &game.player2 {
                write_string(rounds, line, 7, &player2.name)?;
                write_string(rounds, line, 8, &player2.sectorial_name)?;
            }
            write_optional_u8(rounds, line, 9, game.p2_op)?;
            write_optional_u16(rounds, line, 10, game.p2_vp)?;
            write_optional_u32(rounds, line, 11, game.p2_tp)?;
            let forfeit = game
                .forfeit_player
                .and_then(|id| {
                    if game.player1.id == id {
                        Some(game.player1.name.as_str())
                    } else {
                        game.player2.as_ref().map(|player| player.name.as_str())
                    }
                })
                .unwrap_or("");
            write_string(rounds, line, 12, forfeit)?;
            line += 1;
        }
    }
    rounds
        .set_column_width(2, 28)
        .map_err(|error| error.to_string())?;
    rounds
        .set_column_width(7, 28)
        .map_err(|error| error.to_string())?;

    workbook.save_to_buffer().map_err(|error| error.to_string())
}

fn write_string(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &str,
) -> Result<(), String> {
    sheet
        .write_string(row, column, value)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn write_number(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: f64,
) -> Result<(), String> {
    sheet
        .write_number(row, column, value)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn write_optional_u8(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: Option<u8>,
) -> Result<(), String> {
    match value {
        Some(value) => write_number(sheet, row, column, f64::from(value)),
        None => write_string(sheet, row, column, ""),
    }
}

fn write_optional_u16(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: Option<u16>,
) -> Result<(), String> {
    match value {
        Some(value) => write_number(sheet, row, column, f64::from(value)),
        None => write_string(sheet, row, column, ""),
    }
}

fn write_optional_u32(
    sheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: Option<u32>,
) -> Result<(), String> {
    match value {
        Some(value) => write_number(sheet, row, column, f64::from(value)),
        None => write_string(sheet, row, column, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::apply;
    use crate::model::Action;

    #[test]
    fn workbook_is_an_xlsx_archive() {
        let tournament = apply(
            None,
            Action::Create {
                id: "x".into(),
                name: "Export".into(),
                rounds: 1,
                tables: 1,
            },
        )
        .unwrap();
        let bytes = export_xlsx(&tournament).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }
}
