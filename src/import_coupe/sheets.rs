use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use urlencoding::encode;

use crate::store::normalize_name;

pub const POOL_SHEETS: [&str; 4] = ["Bassin A", "Bassin B", "Bassin C", "Bassin D"];

#[derive(Debug, Clone)]
pub struct PoolMatchLine {
    pub player1: String,
    pub player2: String,
    pub p1_obj: u8,
    pub p2_obj: u8,
    pub p1_surv: u16,
    pub p2_surv: u16,
    pub scenario: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PoolStanding {
    pub player: String,
    pub faction: String,
    pub points: u32,
    pub objectives: u32,
    pub survivors: u32,
}

#[derive(Debug, Clone)]
pub struct BracketMatchLine {
    pub player1: String,
    pub player2: String,
    pub p1_obj: u8,
    pub p2_obj: u8,
    pub p1_surv: u16,
    pub p2_surv: u16,
    pub scenario: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedCoupeSheet {
    pub inscriptions: Vec<String>,
    pub pools: Vec<Vec<String>>,
    pub pool_matches: HashMap<(String, String), PoolMatchLine>,
    pub pool_standings: HashMap<String, PoolStanding>,
    /// Barrages 2e vs 3e (format round_of_16 uniquement).
    pub bracket_r16: Vec<BracketMatchLine>,
    pub bracket_quarters: Vec<BracketMatchLine>,
    pub bracket_semis: Vec<BracketMatchLine>,
    pub bracket_final: Option<BracketMatchLine>,
    pub final_placements: Vec<(String, u32)>,
}

pub fn fetch_sheet(client: &Client, spreadsheet_id: &str, sheet_name: &str) -> Result<Vec<Vec<String>>> {
    let url = format!(
        "https://docs.google.com/spreadsheets/d/{spreadsheet_id}/gviz/tq?tqx=out:csv&sheet={}",
        encode(sheet_name)
    );
    let body = client
        .get(&url)
        .send()
        .with_context(|| format!("téléchargement feuille {sheet_name}"))?
        .error_for_status()
        .with_context(|| format!("feuille {sheet_name} inaccessible"))?
        .text()?;

    let mut rows = Vec::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(body.as_bytes());
    for record in reader.records() {
        rows.push(record?.iter().map(|s| s.trim().to_string()).collect());
    }
    Ok(rows)
}

pub fn load_coupe_data(client: &Client, spreadsheet_id: &str) -> Result<ParsedCoupeSheet> {
    let recap = fetch_sheet(client, spreadsheet_id, "Récapitulatif")?;
    let mut inscriptions = parse_inscriptions(&recap);
    let pools = parse_pool_rosters(&recap);
    let mut pool_standings = parse_pool_standings_from_recap(&recap);
    let final_placements = parse_final_placements(&recap);

    let mut pool_matches = HashMap::new();
    for sheet in POOL_SHEETS {
        let rows = fetch_sheet(client, spreadsheet_id, sheet).with_context(|| {
            format!("feuille {sheet} — vérifiez que l'onglet existe dans le spreadsheet")
        })?;
        for m in parse_pool_sheet_matches(&rows)? {
            let key = match_key(&m.player1, &m.player2);
            pool_matches.insert(key, m);
        }
        for s in parse_pool_sheet_standings(&rows) {
            pool_standings.entry(normalize_name(&s.player)).or_insert(s);
        }
    }

    for pool in &pools {
        for player in pool {
            inscriptions.push(player.clone());
        }
    }
    inscriptions = dedupe_players(inscriptions);

    let mut known: HashSet<String> = inscriptions
        .iter()
        .map(|p| normalize_name(p))
        .collect();
    for key in pool_matches.keys() {
        known.insert(key.0.clone());
        known.insert(key.1.clone());
    }

    let (bracket_r16, bracket_quarters, bracket_semis, bracket_final) =
        parse_bracket_from_recap(&recap, &known);

    Ok(ParsedCoupeSheet {
        inscriptions,
        pools,
        pool_matches,
        pool_standings,
        bracket_r16,
        bracket_quarters,
        bracket_semis,
        bracket_final,
        final_placements,
    })
}

fn parse_inscriptions(rows: &[Vec<String>]) -> Vec<String> {
    let mut out = Vec::new();
    for row in rows {
        if row.len() < 3 {
            continue;
        }
        if row[1].chars().all(|c| c.is_ascii_digit()) && !row[2].is_empty() {
            let name = canonical_player_name(&row[2]);
            if !is_noise_name(&name) {
                out.push(name);
            }
        }
    }
    dedupe_players(out)
}

fn parse_pool_rosters(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    let pool_col = pool_data_start_col(rows);
    let pool_end = pool_col + 5;
    let mut pools: Vec<Vec<String>> = vec![Vec::new(); 4];
    let mut i = 0;
    while i < rows.len() {
        let row = &rows[i];
        let Some(letter) = row_pool_letter(row) else {
            i += 1;
            continue;
        };
        let idx = pool_index(letter);
        i += 1;
        // Ne saute une ligne d'en-tête que si c'est vraiment Joueur/Faction
        // sans classement de poule (évite de zapper « 1er Kamcord | Classement | Joueur »).
        if i < rows.len()
            && rows[i].iter().any(|c| c == "Joueur")
            && player_name_from_standing_row_zone(&rows[i], pool_col, pool_end).is_none()
        {
            i += 1;
        }
        let mut players = Vec::new();
        while i < rows.len() {
            let r = &rows[i];
            if row_pool_letter(r).is_some() {
                break;
            }
            let pool_rank_on_row = player_name_from_standing_row_zone(r, pool_col, pool_end);
            if let Some(name) = pool_rank_on_row.clone() {
                players.push(name);
            }
            if r.iter().any(|c| c.contains("Classement final")) && pool_rank_on_row.is_none() {
                break;
            }
            if r.iter().any(|c| c == "Classement")
                && r.iter().any(|c| c == "Joueur")
                && pool_rank_on_row.is_none()
            {
                break;
            }
            i += 1;
        }
        if !players.is_empty() {
            pools[idx] = players;
        }
    }
    pools
}

fn pool_data_start_col(rows: &[Vec<String>]) -> usize {
    for row in rows.iter().take(20) {
        for (i, c) in row.iter().enumerate() {
            if c.contains("Poule A") || c == "Poule A" {
                return i;
            }
        }
    }
    7
}

fn player_name_from_standing_row_zone(row: &[String], start: usize, end: usize) -> Option<String> {
    let rank_idx = row
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .find_map(|(i, c)| if is_pool_rank(c) { Some(i) } else { None })?;
    let name = canonical_player_name(row.get(rank_idx + 1)?);
    if is_noise_name(&name) {
        None
    } else {
        Some(name)
    }
}

fn row_pool_letter(row: &[String]) -> Option<char> {
    row.iter().find_map(|c| pool_letter_from_cell(c))
}

fn pool_letter_from_cell(cell: &str) -> Option<char> {
    let cell = cell.trim();
    for prefix in ["Poule ", "Bassin "] {
        if let Some(pos) = cell.find(prefix) {
            let rest = cell[pos + prefix.len()..].trim();
            if let Some(ch) = rest.chars().next() {
                if ch.is_ascii_alphabetic() {
                    return Some(ch.to_ascii_uppercase());
                }
            }
        }
    }
    None
}

fn parse_pool_standings_from_recap(rows: &[Vec<String>]) -> HashMap<String, PoolStanding> {
    let mut out = HashMap::new();
    for row in rows {
        if row.iter().any(|c| is_pool_rank(c)) {
            if let Some(standing) = standing_from_row(row) {
                out.insert(normalize_name(&standing.player), standing);
            }
        }
    }
    out
}

fn parse_pool_sheet_standings(rows: &[Vec<String>]) -> Vec<PoolStanding> {
    rows.iter()
        .filter(|r| r.iter().any(|c| is_pool_rank(c)))
        .filter_map(|r| standing_from_row(r))
        .collect()
}

fn standing_from_row(row: &[String]) -> Option<PoolStanding> {
    let rank_idx = row.iter().position(|c| is_pool_rank(c))?;
    let player_idx = rank_idx + 1;
    if player_idx >= row.len() {
        return None;
    }
    let player = canonical_player_name(&row[player_idx]);
    if is_noise_name(&player) {
        return None;
    }
    let faction = row.get(player_idx + 1).cloned().unwrap_or_default();
    let nums = numeric_tail(&row[player_idx + 2..]);
    if nums.len() < 3 {
        return None;
    }
    Some(PoolStanding {
        player,
        faction,
        points: nums[0],
        objectives: nums[1],
        survivors: nums[2],
    })
}

fn parse_pool_sheet_matches(rows: &[Vec<String>]) -> Result<Vec<PoolMatchLine>> {
    let header_row = rows
        .iter()
        .position(|r| r.iter().any(|c| c.eq_ignore_ascii_case("Joueur 1")))
        .context("colonne Joueur 1 introuvable dans l'onglet poule")?;
    let header = &rows[header_row];
    let p1_idx = header
        .iter()
        .position(|c| c.eq_ignore_ascii_case("Joueur 1"))
        .context("Joueur 1")?;
    let scenario_idx = header
        .iter()
        .position(|c| c.starts_with("Scénario") || c.starts_with("Scenario"))
        .unwrap_or(p1_idx + 8);

    let mut matches = Vec::new();
    for row in rows.iter().skip(header_row + 1) {
        if row.len() <= p1_idx + 6 {
            continue;
        }
        let p1 = canonical_player_name(&row[p1_idx]);
        let p2 = canonical_player_name(&row[p1_idx + 4]);
        if is_noise_name(&p1) || is_noise_name(&p2) {
            continue;
        }
        let p1_obj = parse_u8_cell(&row[p1_idx + 2]);
        let p1_surv = parse_u16_cell(&row[p1_idx + 3]);
        let p2_obj = parse_u8_cell(&row[p1_idx + 6]);
        let p2_surv = parse_u16_cell(&row[p1_idx + 7]);
        let (Some(p1_obj), Some(p1_surv), Some(p2_obj), Some(p2_surv)) =
            (p1_obj, p1_surv, p2_obj, p2_surv)
        else {
            continue;
        };
        let scenario = row.get(scenario_idx).filter(|s| !s.is_empty()).cloned();
        matches.push(PoolMatchLine {
            player1: p1,
            player2: p2,
            p1_obj,
            p2_obj,
            p1_surv,
            p2_surv,
            scenario,
        });
    }
    Ok(matches)
}

#[derive(Clone)]
struct ScoreCell {
    row: usize,
    col: usize,
    player: String,
    obj: u8,
    surv: u16,
}

fn parse_bracket_from_recap(
    rows: &[Vec<String>],
    known: &HashSet<String>,
) -> (
    Vec<BracketMatchLine>,
    Vec<BracketMatchLine>,
    Vec<BracketMatchLine>,
    Option<BracketMatchLine>,
) {
    let min_col = bracket_zone_start_col(rows);
    let mut scores = Vec::new();
    for (row_idx, row) in rows.iter().enumerate() {
        if row.iter().any(|c| c.contains("Classement final")) {
            break;
        }
        let mut col = min_col;
        while col < row.len() {
            if let Some((player, obj, surv, width)) = read_bracket_score(row, col, known) {
                scores.push(ScoreCell {
                    row: row_idx,
                    col,
                    player,
                    obj,
                    surv,
                });
                col += width;
            } else {
                col += 1;
            }
        }
    }

    let mut matches: Vec<(usize, BracketMatchLine)> = Vec::new();
    let mut used = vec![false; scores.len()];
    for i in 0..scores.len() {
        if used[i] {
            continue;
        }
        if let Some(j) = (i + 1..scores.len()).find(|&j| {
            !used[j] && pairs_bracket_score(&scores[i], &scores[j])
        }) {
            used[i] = true;
            used[j] = true;
            let a = &scores[i];
            let b = &scores[j];
            let scenario = find_scenario_between(rows, a.row, b.row, a.col);
            matches.push((
                a.col,
                BracketMatchLine {
                    player1: a.player.clone(),
                    player2: b.player.clone(),
                    p1_obj: a.obj,
                    p2_obj: b.obj,
                    p1_surv: a.surv,
                    p2_surv: b.surv,
                    scenario,
                },
            ));
        }
    }

    matches.sort_by_key(|(col, _)| *col);

    // Regroupe les matchs par colonne (chaque tour de l'arbre).
    let mut groups: Vec<Vec<BracketMatchLine>> = Vec::new();
    let mut current_col: Option<usize> = None;
    for (col, m) in matches {
        if current_col != Some(col) {
            groups.push(Vec::new());
            current_col = Some(col);
        }
        groups.last_mut().unwrap().push(m);
    }

    // round_of_16 : [4 barrages][4 quarts][2 demis][1 finale]
    // quarters_direct : [4 quarts][2 demis][1 finale]
    let (r16, quarters, semis, final_match) = if groups.first().map(|g| g.len()) == Some(4)
        && groups.get(1).map(|g| g.len()) == Some(4)
    {
        let r16 = groups[0].clone();
        let quarters = groups[1].clone();
        let mut rest: Vec<BracketMatchLine> = groups.into_iter().skip(2).flatten().collect();
        let final_match = rest.pop();
        (r16, quarters, rest, final_match)
    } else {
        let mut flat: Vec<BracketMatchLine> = groups.into_iter().flatten().collect();
        let quarters: Vec<_> = flat.drain(..flat.len().min(4)).collect();
        let final_match = flat.pop();
        (Vec::new(), quarters, flat, final_match)
    };

    (r16, quarters, semis, final_match)
}

fn bracket_zone_start_col(rows: &[Vec<String>]) -> usize {
    for row in rows.iter().take(5) {
        for (i, c) in row.iter().enumerate() {
            if c.contains("Arbre final") || c.contains("Arbre #") {
                return i;
            }
        }
    }
    13
}

fn read_bracket_score(
    row: &[String],
    col: usize,
    known: &HashSet<String>,
) -> Option<(String, u8, u16, usize)> {
    if col + 1 >= row.len() {
        return None;
    }
    let name = canonical_player_name(&row[col]);
    if name.is_empty() || is_noise_name(&name) {
        return None;
    }
    let key = normalize_name(&name);
    if !known.contains(&key) && !looks_like_player(&name) {
        return None;
    }
    // Doit être suivi d'une faction (pas d'un chiffre).
    if row[col + 1].is_empty() || row[col + 1].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if is_noise_name(&row[col + 1]) || is_pool_rank(&row[col + 1]) || is_rank_label(&row[col + 1])
    {
        return None;
    }
    // OP/VP peuvent être vides dans les sheets historiques → 0.
    let obj = row.get(col + 2).and_then(|s| parse_u8_cell(s)).unwrap_or(0);
    let surv = row.get(col + 3).and_then(|s| parse_u16_cell(s)).unwrap_or(0);
    Some((name, obj, surv, 4))
}

fn pairs_bracket_score(a: &ScoreCell, b: &ScoreCell) -> bool {
    if a.player == b.player {
        return false;
    }
    a.col == b.col && (a.row as i32 - b.row as i32).abs() <= 2
}

fn find_scenario_between(rows: &[Vec<String>], row_a: usize, row_b: usize, col: usize) -> Option<String> {
    let start = row_a.min(row_b);
    let end = row_a.max(row_b);
    for row in &rows[start..=end.min(rows.len().saturating_sub(1))] {
        for c in col.saturating_sub(2)..row.len().min(col + 6) {
            if let Some(s) = read_scenario_cell(&row[c]) {
                return Some(s);
            }
        }
    }
    None
}

fn read_scenario_cell(cell: &str) -> Option<String> {
    let s = cell.trim();
    if s.is_empty() {
        return None;
    }
    if s.eq_ignore_ascii_case("VP") || s.eq_ignore_ascii_case("OP") {
        return None;
    }
    if s.starts_with("http") {
        return None;
    }
    if s.starts_with("Arbre #") {
        return None;
    }
    if s.contains("Mission ") {
        return Some(s.to_string());
    }
    if s.len() > 3
        && !s.chars().all(|c| c.is_ascii_digit())
        && !is_pool_rank(s)
        && !s.eq_ignore_ascii_case("Classement")
    {
        return Some(s.to_string());
    }
    None
}

fn parse_final_placements(rows: &[Vec<String>]) -> Vec<(String, u32)> {
    let start_col = final_classement_start_col(rows);
    let mut out = Vec::new();
    let mut pending: Option<(u32, usize)> = None;
    for row in rows {
        if let Some((rank, idx)) = row
            .iter()
            .enumerate()
            .skip(start_col)
            .find_map(|(i, c)| {
                if is_rank_label(c) {
                    Some((parse_rank_label(c), i))
                } else {
                    None
                }
            })
        {
            if rank == 99 {
                continue;
            }
            let players = players_from_classement_row(row, idx, rank);
            out.extend(players.clone());
            let expected = expected_slots(rank);
            if players.len() < expected {
                pending = Some((rank + players.len() as u32, idx));
            } else {
                pending = None;
            }
            continue;
        }
        if let Some((next_rank, idx)) = pending {
            let col = idx + 1;
            if col < row.len() {
                let p = canonical_player_name(&row[col]);
                if looks_like_player(&p) && !is_noise_name(&p) {
                    out.push((p, next_rank));
                    pending = None;
                }
            }
        }
    }
    out
}

fn expected_slots(rank: u32) -> usize {
    match rank {
        1 | 2 => 1,
        3 => 2,
        5 | 9 => 4,
        _ => 1,
    }
}

fn players_from_classement_row(row: &[String], rank_idx: usize, rank: u32) -> Vec<(String, u32)> {
    let player_cols: Vec<usize> = match rank {
        1 | 2 => vec![1],
        3 => vec![1, 5],
        5 | 9 => {
            let mut cols = vec![1];
            let mut col = rank_idx + 5;
            while col + 1 < row.len() && cols.len() < 4 {
                cols.push(col - rank_idx);
                col += 4;
            }
            cols
        }
        _ => vec![1],
    };
    let mut out = Vec::new();
    for (i, off) in player_cols.iter().enumerate() {
        let col = rank_idx + off;
        if col >= row.len() {
            continue;
        }
        let p = canonical_player_name(&row[col]);
        if looks_like_player(&p) && !is_noise_name(&p) {
            out.push((p, rank + i as u32));
        }
    }
    out
}

fn final_classement_start_col(rows: &[Vec<String>]) -> usize {
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            if c.contains("Classement final") {
                return i;
            }
        }
    }
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            if c == "Classement" && row.get(i + 1).map(|x| x.as_str()) == Some("Joueur") {
                return i;
            }
        }
    }
    14
}


fn is_rank_label(label: &str) -> bool {
    matches!(
        label,
        "1er" | "2nd" | "2ème" | "3-4ème" | "5-8ème" | "9-12ème"
    )
}

fn parse_rank_label(label: &str) -> u32 {
    match label {
        "1er" => 1,
        "2nd" | "2ème" => 2,
        "3-4ème" => 3,
        "5-8ème" => 5,
        "9-12ème" => 9,
        _ => 99,
    }
}

fn is_pool_rank(s: &str) -> bool {
    matches!(s, "1er" | "2ème" | "3ème" | "4ème" | "5ème" | "6ème")
}

fn pool_index(letter: char) -> usize {
    match letter.to_ascii_uppercase() {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        'D' => 3,
        _ => 0,
    }
}

fn numeric_tail(cells: &[String]) -> Vec<u32> {
    cells
        .iter()
        .filter_map(|c| c.parse::<u32>().ok())
        .collect()
}

fn parse_u8_cell(s: &str) -> Option<u8> {
    s.trim().parse::<u8>().ok()
}

fn parse_u16_cell(s: &str) -> Option<u16> {
    s.trim().parse::<u16>().ok()
}

pub fn match_key(a: &str, b: &str) -> (String, String) {
    let mut keys = [normalize_name(a), normalize_name(b)];
    keys.sort();
    (keys[0].clone(), keys[1].clone())
}

pub fn canonical_player_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let normalized = trimmed.replace('’', "'").replace("Do/Obakami", "Obakami");
    match normalize_name(&normalized).as_str() {
        "mamanpoulet" | "maman poulet" => "Maman_Poulet".into(),
        "maman_poulet" => "Maman_Poulet".into(),
        "logascon" => "LoGascon".into(),
        "gui zou" | "guizou" | "guizou/lepropre" | "guizou / lepropre" => "GuiZou".into(),
        "lepropre" => "Lepropre".into(),
        "fanfoue" | "fanfoue(mercure)" | "fanfoue (mercure)" => "Fanfoué (MErcurE)".into(),
        "miki/greaves" | "miki" => "Miki/Greaves".into(),
        "scorpion" => "-Scorpion-".into(),
        "grdschtrmf" => "GrdSchtrmf".into(),
        "kantain" => "Kantain".into(),
        "akiard" => "Akiard".into(),
        "shas'o kassad" => "Shas'O Kassad".into(),
        "maxgorthor" => "maxgorthor".into(),
        "wulfric" => "wulfric".into(),
        "teralfox" => "TeralFox".into(),
        _ => normalized,
    }
}

fn dedupe_players(names: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in names {
        let key = normalize_name(&name);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        out.push(name);
    }
    out
}

fn is_noise_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.is_empty()
        || n == "joueur"
        || n == "faction"
        || n.starts_with("mission ")
        || n.starts_with("classement")
        || n.starts_with("http")
        || n == "vp"
        || n == "op"
}

fn looks_like_player(name: &str) -> bool {
    let n = name.trim();
    n.len() >= 2
        && !n.chars().all(|c| c.is_ascii_digit())
        && !n.contains("Mission")
        && !n.starts_with("Arbre")
}

pub fn validate_parsed(data: &ParsedCoupeSheet) -> Result<()> {
    if data.pools.len() != 4 {
        bail!(
            "4 poules attendues, {} trouvées",
            data.pools.len()
        );
    }
    if data.inscriptions.is_empty() {
        bail!("aucun inscrit trouvé");
    }
    if data.pool_matches.is_empty() {
        bail!("aucun match de poule trouvé");
    }
    Ok(())
}
