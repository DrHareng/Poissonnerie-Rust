use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::store::normalize_name;

pub const DEFAULT_PACK_SLUG: &str = "poissonnerie-v2";

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioPack {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub version: Option<String>,
    pub preamble_md: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommonRule {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub body_md: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecondaryObjective {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub body_md: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioSummary {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub flavor_text: Option<String>,
    pub map_filename: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioDetail {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub flavor_text: Option<String>,
    pub map_filename: Option<String>,
    pub end_condition_md: Option<String>,
    pub objectives_md: Option<String>,
    pub deployment_notes_md: Option<String>,
    pub exclusion_zones_md: Option<String>,
    pub elements_md: Option<String>,
    pub special_rules_md: Option<String>,
    pub sort_order: i64,
    /// Rappel de la règle commune « Zones d’exclusion », si le scénario en définit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusion_rule: Option<CommonRule>,
    /// Règles communes citées (hors zones d’exclusion, affichées à part).
    pub common_rules: Vec<CommonRule>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioPackPage {
    pub pack: ScenarioPack,
    pub scenarios: Vec<ScenarioSummary>,
}

pub fn seed_default_pack_if_needed(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM scenario_packs WHERE slug = ?1)",
        params![DEFAULT_PACK_SLUG],
        |row| row.get(0),
    )?;
    if exists {
        return Ok(());
    }

    conn.execute(
        "
        INSERT INTO scenario_packs (slug, name, version, preamble_md, sort_order)
        VALUES (?1, ?2, ?3, ?4, 0)
        ",
        params![
            DEFAULT_PACK_SLUG,
            "Pack de Scénario de la Poissonnerie",
            "v2",
            PREAMBLE_MD,
        ],
    )?;
    let pack_id = conn.last_insert_rowid();

    for (i, rule) in COMMON_RULES.iter().enumerate() {
        conn.execute(
            "
            INSERT INTO common_rules (pack_id, slug, name, body_md, sort_order)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![pack_id, rule.0, rule.1, rule.2, i as i64],
        )?;
    }

    for (i, secondary) in SECONDARIES.iter().enumerate() {
        conn.execute(
            "
            INSERT INTO secondary_objectives (pack_id, slug, name, body_md, sort_order)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![pack_id, secondary.0, secondary.1, secondary.2, i as i64],
        )?;
    }

    for (i, scenario) in SCENARIOS.iter().enumerate() {
        let name_key = normalize_name(scenario.name);
        conn.execute(
            "
            INSERT INTO scenarios (
                name, name_key, usage_count, pack_id, slug, map_filename,
                flavor_text, end_condition_md, objectives_md, deployment_notes_md,
                exclusion_zones_md, elements_md, special_rules_md, sort_order
            ) VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ",
            params![
                scenario.name,
                name_key,
                pack_id,
                scenario.slug,
                scenario.map_filename,
                scenario.flavor_text,
                scenario.end_condition_md,
                scenario.objectives_md,
                scenario.deployment_notes_md,
                scenario.exclusion_zones_md,
                nonempty(scenario.elements_md),
                nonempty(scenario.special_rules_md),
                i as i64,
            ],
        )?;
        let scenario_id = conn.last_insert_rowid();
        for rule_slug in scenario.common_rule_slugs {
            conn.execute(
                "
                INSERT INTO scenario_common_rules (scenario_id, common_rule_id)
                SELECT ?1, id FROM common_rules
                WHERE pack_id = ?2 AND slug = ?3
                ",
                params![scenario_id, pack_id, rule_slug],
            )?;
        }
    }

    Ok(())
}

/// Aligne `map_filename` sur les fichiers présents dans `frontend/public/scenario-maps/`.
pub fn sync_map_filenames(conn: &Connection) -> Result<()> {
    for scenario in SCENARIOS {
        if let Some(filename) = scenario.map_filename {
            conn.execute(
                "UPDATE scenarios SET map_filename = ?1 WHERE slug = ?2",
                params![filename, scenario.slug],
            )?;
        }
    }
    Ok(())
}

/// Images de contenu scénario (`frontend/public/scenario/`), pour `[img]fichier[img]`.
pub fn list_scenario_content_images() -> Result<Vec<String>> {
    use std::fs;
    use std::path::PathBuf;

    let candidates = [
        PathBuf::from("frontend/public/scenario"),
        PathBuf::from("public/scenario"),
    ];
    let dir = candidates
        .into_iter()
        .find(|path| path.is_dir())
        .ok_or_else(|| anyhow::anyhow!("dossier frontend/public/scenario introuvable"))?;

    let mut names = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".gif")
            || lower.ends_with(".webp")
            || lower.ends_with(".svg"))
        {
            continue;
        }
        names.push(name.to_string());
    }
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(names)
}

/// Retire « Zones d’exclusion » des liaisons scénario ↔ règles communes
/// (rappel affiché à part, à côté de la carte).
pub fn sync_exclusion_rule_links(conn: &Connection) -> Result<()> {
    conn.execute(
        "
        DELETE FROM scenario_common_rules
        WHERE common_rule_id IN (
            SELECT id FROM common_rules WHERE slug = 'zones-exclusion'
        )
        ",
        [],
    )?;
    Ok(())
}

fn nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn get_pack_page(conn: &Connection, slug: &str) -> Result<Option<ScenarioPackPage>> {
    let Some(pack) = get_pack(conn, slug)? else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "
        SELECT id, slug, name, flavor_text, map_filename, sort_order
        FROM scenarios
        WHERE pack_id = ?1
        ORDER BY sort_order ASC, name ASC
        ",
    )?;
    let scenarios = stmt
        .query_map(params![pack.id], |row| {
            Ok(ScenarioSummary {
                id: row.get(0)?,
                slug: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                name: row.get(2)?,
                flavor_text: row.get(3)?,
                map_filename: row.get(4)?,
                sort_order: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(ScenarioPackPage { pack, scenarios }))
}

pub fn get_pack(conn: &Connection, slug: &str) -> Result<Option<ScenarioPack>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, slug, name, version, preamble_md
        FROM scenario_packs
        WHERE slug = ?1
        ",
    )?;
    let mut rows = stmt.query(params![slug])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(ScenarioPack {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            version: row.get(3)?,
            preamble_md: row.get(4)?,
        }));
    }
    Ok(None)
}

pub fn list_secondaries(conn: &Connection, pack_slug: &str) -> Result<Vec<SecondaryObjective>> {
    let mut stmt = conn.prepare(
        "
        SELECT so.id, so.slug, so.name, so.body_md
        FROM secondary_objectives so
        JOIN scenario_packs p ON p.id = so.pack_id
        WHERE p.slug = ?1
        ORDER BY so.sort_order ASC, so.name ASC
        ",
    )?;
    let rows = stmt.query_map(params![pack_slug], |row| {
        Ok(SecondaryObjective {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            body_md: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn list_common_rules(conn: &Connection, pack_slug: &str) -> Result<Vec<CommonRule>> {
    let mut stmt = conn.prepare(
        "
        SELECT cr.id, cr.slug, cr.name, cr.body_md
        FROM common_rules cr
        JOIN scenario_packs p ON p.id = cr.pack_id
        WHERE p.slug = ?1
        ORDER BY cr.sort_order ASC, cr.name ASC
        ",
    )?;
    let rows = stmt.query_map(params![pack_slug], |row| {
        Ok(CommonRule {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            body_md: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_scenario_detail(conn: &Connection, slug: &str) -> Result<Option<ScenarioDetail>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, slug, name, flavor_text, map_filename,
               end_condition_md, objectives_md, deployment_notes_md,
               exclusion_zones_md, elements_md, special_rules_md, sort_order, pack_id
        FROM scenarios
        WHERE slug = ?1
        ",
    )?;
    let mut rows = stmt.query(params![slug])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let scenario_id: i64 = row.get(0)?;
    let exclusion_zones_md: Option<String> = row.get(8)?;
    let pack_id: Option<i64> = row.get(12)?;
    let mut detail = ScenarioDetail {
        id: scenario_id,
        slug: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        name: row.get(2)?,
        flavor_text: row.get(3)?,
        map_filename: row.get(4)?,
        end_condition_md: row.get(5)?,
        objectives_md: row.get(6)?,
        deployment_notes_md: row.get(7)?,
        exclusion_zones_md: exclusion_zones_md.clone(),
        elements_md: row.get(9)?,
        special_rules_md: row.get(10)?,
        sort_order: row.get(11)?,
        exclusion_rule: None,
        common_rules: Vec::new(),
    };

    if exclusion_zones_md.is_some() {
        if let Some(pack_id) = pack_id {
            let mut rule_stmt = conn.prepare(
                "
                SELECT id, slug, name, body_md
                FROM common_rules
                WHERE pack_id = ?1 AND slug = 'zones-exclusion'
                LIMIT 1
                ",
            )?;
            let mut rule_rows = rule_stmt.query(params![pack_id])?;
            if let Some(rule_row) = rule_rows.next()? {
                detail.exclusion_rule = Some(CommonRule {
                    id: rule_row.get(0)?,
                    slug: rule_row.get(1)?,
                    name: rule_row.get(2)?,
                    body_md: rule_row.get(3)?,
                });
            }
        }
    }

    let mut rules_stmt = conn.prepare(
        "
        SELECT cr.id, cr.slug, cr.name, cr.body_md
        FROM common_rules cr
        JOIN scenario_common_rules scr ON scr.common_rule_id = cr.id
        WHERE scr.scenario_id = ?1
          AND cr.slug != 'zones-exclusion'
        ORDER BY cr.name COLLATE NOCASE ASC
        ",
    )?;
    detail.common_rules = rules_stmt
        .query_map(params![scenario_id], |row| {
            Ok(CommonRule {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                body_md: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(detail))
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePackRequest {
    pub preamble_md: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateNamedMdRequest {
    pub name: String,
    pub body_md: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateScenarioContentRequest {
    pub flavor_text: Option<String>,
    pub end_condition_md: Option<String>,
    pub objectives_md: Option<String>,
    pub deployment_notes_md: Option<String>,
    pub exclusion_zones_md: Option<String>,
    pub elements_md: Option<String>,
    pub special_rules_md: Option<String>,
}

pub fn update_pack_preamble(
    conn: &Connection,
    slug: &str,
    preamble_md: &str,
) -> Result<Option<ScenarioPack>> {
    let updated = conn.execute(
        "UPDATE scenario_packs SET preamble_md = ?1 WHERE slug = ?2",
        params![preamble_md, slug],
    )?;
    if updated == 0 {
        return Ok(None);
    }
    get_pack(conn, slug)
}

pub fn update_secondary(
    conn: &Connection,
    pack_slug: &str,
    secondary_slug: &str,
    name: &str,
    body_md: &str,
) -> Result<Option<SecondaryObjective>> {
    let updated = conn.execute(
        "
        UPDATE secondary_objectives
        SET name = ?1, body_md = ?2
        WHERE slug = ?3
          AND pack_id = (SELECT id FROM scenario_packs WHERE slug = ?4)
        ",
        params![name.trim(), body_md, secondary_slug, pack_slug],
    )?;
    if updated == 0 {
        return Ok(None);
    }
    let mut stmt = conn.prepare(
        "
        SELECT so.id, so.slug, so.name, so.body_md
        FROM secondary_objectives so
        JOIN scenario_packs p ON p.id = so.pack_id
        WHERE p.slug = ?1 AND so.slug = ?2
        ",
    )?;
    let mut rows = stmt.query(params![pack_slug, secondary_slug])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(SecondaryObjective {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            body_md: row.get(3)?,
        }));
    }
    Ok(None)
}

pub fn update_common_rule(
    conn: &Connection,
    pack_slug: &str,
    rule_slug: &str,
    name: &str,
    body_md: &str,
) -> Result<Option<CommonRule>> {
    let updated = conn.execute(
        "
        UPDATE common_rules
        SET name = ?1, body_md = ?2
        WHERE slug = ?3
          AND pack_id = (SELECT id FROM scenario_packs WHERE slug = ?4)
        ",
        params![name.trim(), body_md, rule_slug, pack_slug],
    )?;
    if updated == 0 {
        return Ok(None);
    }
    let mut stmt = conn.prepare(
        "
        SELECT cr.id, cr.slug, cr.name, cr.body_md
        FROM common_rules cr
        JOIN scenario_packs p ON p.id = cr.pack_id
        WHERE p.slug = ?1 AND cr.slug = ?2
        ",
    )?;
    let mut rows = stmt.query(params![pack_slug, rule_slug])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(CommonRule {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            body_md: row.get(3)?,
        }));
    }
    Ok(None)
}

pub fn update_scenario_content(
    conn: &Connection,
    pack_slug: &str,
    scenario_slug: &str,
    patch: &UpdateScenarioContentRequest,
) -> Result<Option<ScenarioDetail>> {
    let pack_id: i64 = match conn.query_row(
        "SELECT id FROM scenario_packs WHERE slug = ?1",
        params![pack_slug],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(error.into()),
    };

    let Some(mut detail) = get_scenario_detail(conn, scenario_slug)? else {
        return Ok(None);
    };

    let belongs: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM scenarios WHERE slug = ?1 AND pack_id = ?2)",
        params![scenario_slug, pack_id],
        |row| row.get(0),
    )?;
    if !belongs {
        return Ok(None);
    }

    if let Some(value) = &patch.flavor_text {
        detail.flavor_text = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.end_condition_md {
        detail.end_condition_md = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.objectives_md {
        detail.objectives_md = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.deployment_notes_md {
        detail.deployment_notes_md = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.exclusion_zones_md {
        detail.exclusion_zones_md = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.elements_md {
        detail.elements_md = nonempty(value).map(str::to_string);
    }
    if let Some(value) = &patch.special_rules_md {
        detail.special_rules_md = nonempty(value).map(str::to_string);
    }

    conn.execute(
        "
        UPDATE scenarios SET
            flavor_text = ?1,
            end_condition_md = ?2,
            objectives_md = ?3,
            deployment_notes_md = ?4,
            exclusion_zones_md = ?5,
            elements_md = ?6,
            special_rules_md = ?7
        WHERE slug = ?8 AND pack_id = ?9
        ",
        params![
            detail.flavor_text,
            detail.end_condition_md,
            detail.objectives_md,
            detail.deployment_notes_md,
            detail.exclusion_zones_md,
            detail.elements_md,
            detail.special_rules_md,
            scenario_slug,
            pack_id,
        ],
    )?;

    get_scenario_detail(conn, scenario_slug)
}

const PREAMBLE_MD: &str = r#"## Parties

Chaque partie se joue en essayant de marquer plus de points d’objectif que son adversaire. Ces points peuvent être remportés en remplissant deux objectifs différents :

- un **objectif principal**, décrit par le scénario, pour **7** points d’objectifs
- un **objectif secondaire**, choisi parmi 3 objectifs tirés au sort, pour **3** points d’objectifs

## Objectifs secondaires

Au début de la partie, avant de choisir une liste, chaque joueur pioche 3 objectifs secondaires parmi la liste du pack et en choisit un qu’il devra accomplir durant la partie. Il n’est possible de marquer que 3 points par objectif secondaire.

Il n’est pas possible d’interagir avec les pions, civils, et autres éléments qui sont placés sur la table par l’objectif secondaire de votre adversaire.

## Calcul des points de tournoi (TP)

- Victoire (plus de points d’objectif cumulés) : **3 TP**
- Égalité (autant de points d’objectif cumulés) : **1 TP**
- Défaite (moins de points d’objectif cumulés) : **0 TP**
- Défaite avec 2 points d’écart ou moins : **+1 TP**
- Plus de 5 points d’objectif cumulés : **+1 TP**
"#;

const COMMON_RULES: &[(&str, &str, &str)] = &[
    (
        "zones-exclusion",
        "Zones d’exclusion",
        "Aucune combattant ne peut utiliser une compétence avec le label Airborne Deployment (AD) ou Superior Deployment (Forward Deployment, Infiltration, Impersonate) afin de se déployer dans l’une de ces zones.",
    ),
    (
        "civils",
        "Civils",
        r#"Les civils sont des pions avec une valeur de silhouette de 2 et répondant aux règles suivantes :

- pas d’ORA
- ne peut pas être allongé ou engagé
- ne bloque pas : les lignes de vue, les déplacements, les gabarits

### Rallier un civil (ordre court : attaque)

- **Qui ?** Un combattant en contact avec un civil. Le civil ne doit pas être en contact avec un ennemi ou être Rallié. Interdit aux combattants impétueux (même via Frenzy), isolés, immobilisés, périphériques ou REM.
- **Quoi ?** Le combattant fait un jet de WIP.
- **Réussite :** Le Civil est Rallié et répond désormais aux règles de Prise (2, 1 si le combattant fait partie d’une FT). Il ne pourra pas être « ramassé » normalement, seulement par l’action Rallier. L’état Rallié est annulé si le porteur devient impétueux, isolé, immobilisé ou entre en null ou token state."#,
    ),
    (
        "specialistes",
        "Spécialistes",
        "Les combattants ayant l’une des compétences suivantes sont considérés comme spécialistes : Hacker, Doctor, Paramedic, Engineer, Forward Observer, Chain of Command ou Specialist Operative.",
    ),
    (
        "prise",
        "Prise (X)",
        r#"Un certain nombre d’éléments de scénarios peuvent être ramassés :

- Par défaut, tout pion suivant les règles de prise a une silhouette 0 (⌀ 25mm, h 3mm)
- Il peut être soit porté (par un combattant) soit « au sol »
- Chaque combattant peut en porter X maximum (+1 s’il a la compétence baggage)
- S’il est porté : il tombe au sol si le porteur entre en null ou token state ; il peut être posé au sol durant l’activation ; il se déplace avec le combattant à son contact

### Ramasser (ordre court : attaque)

- **Qui ?** Un combattant en contact avec cet élément au sol, ou en contact avec un ami portant cet élément.
- **Quoi ?** Le combattant ramasse l’élément."#,
    ),
    (
        "activable",
        "Activable (qui, bonus : X)",
        r#"Les éléments activables peuvent être activés durant la partie via l’action ci-dessous.

### Activer (ordre court : attaque)

- **Qui ?** Si l’élément a une silhouette : un profil mentionné à son contact. Sinon, le profil mentionné doit avoir plus de la moitié de son socle dedans.
- **Quoi ?** Le combattant fait un jet de WIP. S’il remplit les conditions mentionnées, il aura WIP +3 et lancera 2 dés pour ce test.
- **Réussite :** L’élément est activé, le scénario précisera les effets qui en découlent."#,
    ),
    (
        "controle",
        "Contrôle (X)",
        r#"Certains éléments de scénario apportent des points ou d’autres bonus au joueur les contrôlant. X représente la zone dans laquelle un combattant doit se trouver pour contester le contrôle de cet élément.

Le joueur qui contrôle cet élément est le joueur qui a le plus de Points d’Armée dans la zone. Les combattants comptant doivent :

- Avoir plus de la moitié de son socle dans la zone (sauf si le contrôle est en contact)
- Ne pas être en null state (les combattants en état de Shasvastii-Embryo comptent)
- Pouvoir être en état de marqueur ou être un peripheral"#,
    ),
    (
        "destructible",
        "Destructible",
        r#"Certains éléments de scénario sont destructibles :

- ils ne peuvent être ciblés que par des armes disposant du trait « anti-matériel »
- il est possible de déposer une charge creuse, sans effectuer de jet, comme sur un élément de décors
- ils ne peuvent être détruits qu’au corps à corps ou en posant des D-charges ; ils ne pourront pas être pris pour cible par une BS attack
- s’ils perdent leur dernier point de Structure, ils sont immédiatement retirés de la table
- ils peuvent être réparés normalement via la compétence ingénieur"#,
    ),
];

const SECONDARIES: &[(&str, &str, &str)] = &[
    (
        "enlevement",
        "Enlèvement",
        r#"- Au début de son déploiement, votre adversaire place 2 Civils à plus de 15 cm / 6” de sa zone de déploiement et à plus de 20 cm / 8” l’un de l’autre
- **Fin de partie :**
  - 1 point d’objectif par Civil rallié
  - 1 point si au moins l’un des Civils est dans votre zone de déploiement

*Ces éléments sont à déployer au sol et doivent pouvoir être accessibles.*"#,
    ),
    (
        "saisie-de-materiel",
        "Saisie de matériel",
        r#"- Au début de son déploiement, votre adversaire place 2 pions Matériel à plus de 15 cm / 6” de sa zone de déploiement et à plus de 20 cm / 8” l’un de l’autre
- Les pions Matériel répondent aux règles de Prise (1), mais seuls les spécialistes sont autorisés à ramasser le Matériel
- **Fin de partie :**
  - 1 point d’objectif par Matériel porté par une de vos troupes
  - 1 point d’objectif si un Matériel est dans votre zone de déploiement

*Ces éléments sont à déployer au sol et doivent pouvoir être accessibles.*"#,
    ),
    (
        "soif-de-sang",
        "Soif de sang",
        r#"- **Fin de partie :**
  - 1 point d’objectif par combattant ennemi différent tué ou rendu inconscient suite à une attaque au corps à corps
  - Les Coups de Grâce ne comptent pas, mais une figurine inconsciente tuée par une attaque au corps à corps si."#,
    ),
    (
        "ciblage-orbital",
        "Ciblage orbital",
        r#"- Le fond de table adverse est découpé en 3 Zones à Cibler de 40x40 cm / 16”x16”
- Après la pose de votre réserve, donnez 3 pions Balise à 3 de vos combattants situés dans votre Zone de Déploiement
- Les pions Balises répondent aux règles de Prise (1) et ne peuvent donc pas être donnés à des combattants disposant de la compétence Aerial
- **Fin de partie :** 1 point d’objectif pour chacune des 3 Zones à Cibler qui contient une Balise"#,
    ),
    (
        "reconnaissance-poussee",
        "Reconnaissance poussée",
        r#"- Le fond de table adverse est découpé en 3 Zones à Explorer de 40x40 cm / 16”x16”
- Chaque Zone a Activable (combattant non-périphérique)
- **Réussite :** La Zone est explorée
- **Fin de partie :** 1 point pour chacune des 3 Zones qui a été explorée"#,
    ),
    (
        "vol-informations",
        "Vol d’informations",
        r#"- Les combattants en null ou immobilized state ennemis gagnent Activable (combattant non-périphérique)
- **Réussite :** Le combattant ennemi est interrogé
- À la fin de la partie, vos combattants peuvent activer un ennemi activable en contact
- **Fin de partie :** 1 point d’objectif par combattant ennemi interrogé (même s’il a été retiré de la table)"#,
    ),
    (
        "tete-de-pont",
        "Tête de pont",
        r#"- Au début de votre déploiement, placez votre Tête de Pont (gabarit circulaire)
  - Intégralement à plus de 10 cm / 4” de votre moitié de table
  - Des obstacles peuvent se trouver dessus si le volume dans la zone tient intégralement dans une s2 ou une s3
  - Il est interdit de placer la Tête de Pont sur (même partiellement) un objectif principal
- **Fin de chacun de vos tours :** 1 point d’objectif si vous contrôlez votre Tête de Pont"#,
    ),
    (
        "investigation",
        "Investigation",
        r#"- Au début de votre déploiement, placez le Lieu à Investiguer (gabarit circulaire)
  - Intégralement à plus de 10 cm / 4” de votre moitié de table
  - Des obstacles peuvent se trouver dessus si le volume dans le lieu tient intégralement dans une s2 ou une s3
  - Il est interdit de placer le Lieu à Investiguer sur (même partiellement) un objectif principal
- Le Lieu à Investiguer a Activable (combattant non-périphérique)
- **Réussite :** Le Lieu est investigué
- **Fin de chacun de vos tours :** 1 point d’objectif si vous avez investigué le Lieu"#,
    ),
];

struct ScenarioSeed {
    slug: &'static str,
    name: &'static str,
    map_filename: Option<&'static str>,
    flavor_text: Option<&'static str>,
    end_condition_md: &'static str,
    objectives_md: &'static str,
    deployment_notes_md: Option<&'static str>,
    exclusion_zones_md: Option<&'static str>,
    elements_md: &'static str,
    special_rules_md: &'static str,
    common_rule_slugs: &'static [&'static str],
}

const SCENARIOS: &[ScenarioSeed] = &[
    ScenarioSeed {
        slug: "a-laube-de-linfestation",
        name: "À l’aube de l’infestation",
        map_filename: Some("Infestation.png"),
        flavor_text: Some(
            "Attention. Attention. Tout le personnel doit évacuer immédiatement. Ceci n’est pas un exercice.",
        ),
        end_condition_md: "Fin du troisième round, pas de retraite !",
        objectives_md: r#"- **Fin de tour :** 1 point pour le joueur actif s’il contrôle sa Zone d’Évacuation
- **Fin de round :** 1 point si vous avez strictement plus d’Échantillons portés ou dans votre Zone d’Évacuation que votre adversaire
- **Fin de partie :** 1 point si vous avez strictement plus de points de survivant dans votre Zone d’Évacuation que votre adversaire"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("4” de l’antenne et des tech-coffins"),
        elements_md: r#"### Scientifique Civil (1 par joueur)
Au déploiement, chaque joueur place son scientifique soit dans sa zone de déploiement, soit directement Rallié à un combattant.

### Antenne S3 (x1)
- Activable (spécialiste, bonus : Forward Observer)
- **Réussite :** le joueur actif pose un gabarit circulaire, c’est sa Zone d’Évacuation. Le centre du gabarit doit être dans la moitié de table adverse et à plus de 12”/30cm de la zone d’évacuation adverse. Des obstacles peuvent se trouver dans la zone d’évacuation si le volume dans la zone tient intégralement dans une s2 ou une s3.

### Zone d’Évacuation
Contrôle (le gabarit)

### Tech-coffins S3 (x2)
- Activable (spécialiste, bonus : contrôle un Scientifique)
- **Réussite :** le combattant prend un Échantillon (max 1 par tech-coffin et par tour)

### Échantillon
Prise (1)"#,
        special_rules_md: "",
        common_rule_slugs: &["civils", "specialistes", "activable", "controle", "prise"],
    },
    ScenarioSeed {
        slug: "age-de-glace",
        name: "Âge de glace",
        map_filename: Some("AgeDeGlace.png"),
        flavor_text: Some("Et dire que je trouvais qu’on se les gelait sur Svalarheima."),
        end_condition_md: "Fin du troisième round, PAS DE RETRAITE !",
        objectives_md: r#"- **Fin du round :** 1 point si vous contrôlez strictement plus d’Unités de Chauffage actives que votre adversaire
- **Fin de partie :**
  - 1 point par Chauffage Portatif adverse inactif ou détruit
  - 1 point si au moins un combattant adverse hors équipement et null-state a subi au moins une blessure à cause de la règle Froid Extrême"#,
        deployment_notes_md: None,
        exclusion_zones_md: None,
        elements_md: r#"### Unités de Chauffage S3 (x7)
- Contrôle (contact, uniquement si l’unité de chauffage est activée)
- Inactive au début de la partie
- Activable (spécialiste, bonus : ingénieur)
- **Réussite :** l’unité de chauffage est activée ou désactivée

### Chauffages portatifs S1 (3 par joueur)
- Chaque joueur distribue 3 chauffages portatifs parmi ses combattants à la fin de la phase de déploiement, par ordre d’initiative
- Prise (1)
- Si ce chauffage est au sol, il devient inactif
- Destructible : STR 1, ARM 2"#,
        special_rules_md: r#"### Froid extrême
À la fin du tour de chaque joueur, pour chacun de ses combattants ou équipement qui n’est pas dans la zone de contrôle (8”) d’une unité de chauffage ou d’un chauffage portatif actif :

- il subit une touche PS=7, normale, sur la BTS
- un combattant qui n’est pas en null ou immobilized state peut effectuer un test de PHY pour annuler cette touche"#,
        common_rule_slugs: &["specialistes", "activable", "controle", "prise", "destructible"],
    },
    ScenarioSeed {
        slug: "audit-de-securite",
        name: "Audit de sécurité",
        map_filename: Some("AuditDeSecurite.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous avez strictement plus de Serveurs actifs que votre adversaire
- **Fin de partie :** 1 point par Serveur actif"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque serveur"),
        elements_md: r#"### Employés Civil (x3)

### Serveurs S3 (x4)
Une fois qu’un joueur a rallié un Employé, pour lui, les serveurs gagnent :
- Activable (spécialiste, bonus : combattant avec un employé rallié)
- **Réussite :** si le serveur était activé par un joueur, il ne l’est plus ; sinon il est activé par le joueur"#,
        special_rules_md: "",
        common_rule_slugs: &["civils", "specialistes", "activable"],
    },
    ScenarioSeed {
        slug: "avant-poste",
        name: "Avant-poste",
        map_filename: Some("AvantPoste.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin du tour de chaque joueur :** 1 point pour le joueur actif s’il contrôle strictement plus de Sites
- **Fin de partie :** 1 point par Console active"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("les Sites"),
        elements_md: r#"### Sites gabarit circulaire (x3)
Contrôle (rez de chaussée du bâtiment / de la pièce)

### Consoles S3 (x4)
- Activable (spécialiste, bonus : hacker)
- **Réussite :** si l’antenne était activée pour un joueur, elle ne l’est plus ; sinon elle est activée pour le joueur"#,
        special_rules_md: "",
        common_rule_slugs: &["specialistes", "activable", "controle"],
    },
    ScenarioSeed {
        slug: "bandes-rivales",
        name: "Bandes rivales",
        map_filename: Some("BandesRivales.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de tour :** 1 point si le joueur actif contrôle ou a fouillé la Planque adverse ce tour-ci
- **Fin de round :** 1 point si le commanditaire adverse est rallié ou dans votre ZD
- **Fin de partie :** 1 point pour avoir tué les 2 CSU"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("4” de la planque adverse et l’intérieur de votre propre planque"),
        elements_md: r#"### Planque Objective Room (1 par joueur)
- Chaque porte de la planque est fermée
- Les portes ne permettent qu’à des combattants avec une silhouette de 5 ou moins de rentrer
- Activable (combattant équipé d’une D-charge) — réussit automatiquement : le combattant perd une utilisation de D-charge, la porte est détruite
- Activable (spécialiste au contact d’une porte, bonus : hacker) — ouvre ou ferme toutes les portes (sauf détruites)
- Activable (spécialiste intégralement à l’intérieur) — le joueur a fouillé la planque pour ce tour

### Commanditaire Civil (1 par joueur)

### CSU (2 par joueur)
- ne génère ni ne peut utiliser d’ordre
- déploiement durant la phase de déploiement du joueur, intégralement dans les 4” ou à l’intérieur de votre planque
- peut être déployé allongé"#,
        special_rules_md: "",
        common_rule_slugs: &["civils", "specialistes", "activable"],
    },
    ScenarioSeed {
        slug: "cambriolage",
        name: "Cambriolage",
        map_filename: Some("Cambriolage.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous portez strictement plus de Prototypes que votre adversaire
- **Fin de partie :**
  - 1 point si vous portez au moins un Prototype
  - 1 point si vous avez strictement plus de Munitions Améliorées que votre adversaire sur des combattants hors null-state (Shasvastii-Embryo n’est pas considéré comme null ici)
  - 1 point par Panoplie contrôlée"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque tech-coffin et panoplie"),
        elements_md: r#"### Tech-coffins S3 (x3)
- Activable (spécialiste, bonus : docteur ou paramédic)
- **Réussite :** le combattant récupère un Prototype ; le tech-coffin est retiré

### Prototypes (x3)
Prise (1)

### Panoplies S3 (x2)
- Activable (non périphérique, bonus : forward observer)
- **Réussite :** le combattant reçoit un état Munitions Améliorées

### Munitions améliorées
- Le combattant gagne BS Attack (SR-1)
- limite 1 par combattant
- état permanent"#,
        special_rules_md: "",
        common_rule_slugs: &["specialistes", "activable", "prise", "controle"],
    },
    ScenarioSeed {
        slug: "contacter-le-qg",
        name: "Contacter le QG",
        map_filename: Some("ContacterLeQG.png"),
        flavor_text: Some("Églantine ? Ici Mirabelle…"),
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous avez réalisé plus ou autant de Tâches que votre adversaire durant la partie (min. 1 Classifié réalisé)
- **Fin de partie :**
  - 1 point par Tâche réalisée de plus que votre adversaire (max. 3)
  - 1 point si vous avez réparé plus de Radios que votre adversaire"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque radio"),
        elements_md: r#"### Radios endommagées S3 (x4)
- Activable (spécialiste, bonus : engineer)
- **Réussite :** La Radio est réparée, son activation est modifiée

### Radio réparée S3
- Activable (non périphérique, bonus : spécialiste)
- **Réussite :** Le joueur pioche 2 Tâches et en garde 1 (maximum 1 fois par radio et par tour)"#,
        special_rules_md: r#"### Tâches
Une fois que chaque joueur a choisi son secondaire, le reste de chaque deck d’objectifs secondaires (incluant le secondaire qui n’a pas été pris) est mélangé pour former une pioche de Tâches.

### Piocher une Tâche
Quand un joueur pioche une Tâche, il la révèle et lui ou son adversaire place immédiatement les éléments associés. Au début de la partie, chaque joueur pioche 3 Tâches et en garde 2.

### Réaliser une Tâche
Une Tâche est réalisée lorsqu’à la fin du tour du joueur il remplit les conditions pour marquer un de ses points d’objectif (même si la condition indique fin de partie). Après avoir pioché 1 Tâche, si un joueur en a plus que 4 (réalisés + en main), il devra en défausser 1. Une fois réalisée, enlevez les éléments liés à sa réalisation."#,
        common_rule_slugs: &["specialistes", "activable"],
    },
    ScenarioSeed {
        slug: "controler-les-communications",
        name: "Contrôler les communications",
        map_filename: Some("ControlerLesCommunications.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous avez plus ou autant d’Objectifs actifs que votre adversaire (min. 1 objectif actif)
- **Fin de partie :** 1 point par Objectif actif"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque objectif"),
        elements_md: r#"### Objectifs S3 (x6)
- Consoles : Activable (spécialiste, bonus hacker)
- Antennes : Activable (spécialiste, bonus engineer)
- **Réussite :** si l’objectif était activé par un joueur, il ne l’est plus ; sinon il est activé par le joueur"#,
        special_rules_md: "",
        common_rule_slugs: &["specialistes", "activable"],
    },
    ScenarioSeed {
        slug: "data-mining",
        name: "Data Mining",
        map_filename: Some("DataMining.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous avez plus ou autant de Serveurs actifs et non-détruits que votre adversaire (min. 1 serveur actif)
- **Fin de partie :**
  - 1 point par Superviseur ennemi rallié ou dans votre ZD
  - 1 point par Serveur activé (détruit ou non)"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque serveur"),
        elements_md: r#"### Superviseur Civil (2 par joueur)
Après le jet de lieutenant, avant le déploiement, chaque joueur place ses 2 superviseurs à plus de 10 cm de sa ZD et à plus de 30 cm les uns des autres. Chacun détient un Code qui peut être dérobé.

### Serveurs S3 (x4)
- Activable (spécialiste, bonus : hacker)
- Précondition : le joueur doit avoir un Code
- **Réussite :** le Code est défaussé ; ce serveur est activé pour le joueur et ne pourra plus l’être ; le Serveur devient Destructible : STR 2, ARM 4"#,
        special_rules_md: r#"### Rallier / dérober un Code
Si un spécialiste rallie un Superviseur ennemi avec un Code, le Code est volé. Un Master Breacher ou hacker en ZC d’un Superviseur ennemi avec Code peut dérober le Code (ordre court : attaque, WIP -3).

### Master breacher
À la fin du déploiement de sa réserve, un joueur désigne un de ses combattants déployé non-token, non REM, non VEH et non-TAG comme master breacher. Il reçoit des charges creuses et Specialist Operative."#,
        common_rule_slugs: &["civils", "specialistes", "activable", "destructible"],
    },
    ScenarioSeed {
        slug: "epidemie",
        name: "Épidémie",
        map_filename: Some("Epidemie.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de partie :**
  - 1 point par Infectés adverse avec le statut Soigné
  - 1 point par Tech-Coffin contrôlé
  - 1 point si vous avez récupéré plus ou autant de Remèdes que votre adversaire
  - 1 point si vous avez moins de combattants Contaminés présents sur la table que votre adversaire
  - 1 point par joueur si aucun combattant n’est Contaminé"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque tech-coffin"),
        elements_md: r#"### Infectés Civil (3 par joueur)
Placés après le jet de lieutenant, à plus de 10 cm de la ZD et 30 cm les uns des autres. Ils commencent Contaminés.

### Tech-coffins S3 (x2)
- Contrôle (contact) — à la fin de votre tour, pour chaque Tech-Coffin contrôlé, un combattant au contact récupère un Remède
- Activable (spécialiste, bonus : docteur ou paramédic) — récupère un Remède (plusieurs fois par tour possibles)

### Remèdes
Prise (2). Administrer un remède (ordre court) : le Contaminé perd ce statut et gagne Soigné ; un Infecté peut être immédiatement rallié."#,
        special_rules_md: r#"### Contamination
Lorsqu’un combattant entre en contact avec un Infecté Contaminé ou un combattant Contaminé (ami ou ennemi), il gagne Contaminé. Un Infecté ou combattant Soigné ne peut plus recevoir Contaminé."#,
        common_rule_slugs: &["civils", "specialistes", "activable", "controle", "prise"],
    },
    ScenarioSeed {
        slug: "exfiltration",
        name: "Exfiltration",
        map_filename: Some("Exfiltration.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin du round :** 1 point si vous avez strictement plus de Cibles ennemies ralliées
- **Fin de partie :**
  - 1 point par CSU tué
  - 1 point si l’une des deux Cibles adverses est dans votre ZD
  - 1 point si vous avez révélé les 2 Cibles et les 2 CSU adverses"#,
        deployment_notes_md: Some(
            "Après le jet de lieutenant et avant le déploiement, CSU et Cibles (Imp-2) se déploient hors de toute ZD, à 8” minimum les uns des autres, accessibles.",
        ),
        exclusion_zones_md: None,
        elements_md: r#"### Cible Civil (2 par joueur)
Déployée avec Impersonation-2 (1 use). Il n’est pas possible de rallier ses propres Cibles.

### CSU (2 par joueur)
Ne génère ni ne peut utiliser d’ordre. Ne peut pas être déployé allongé."#,
        special_rules_md: "",
        common_rule_slugs: &["civils"],
    },
    ScenarioSeed {
        slug: "fouilles-archeologiques",
        name: "Fouilles archéologiques",
        map_filename: Some("FouillesArcheologiques.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :** 1 point si vous avez activé strictement plus d’Excavateurs que votre adversaire ce tour-ci
- **Fin de partie :**
  - 1 point si vous avez au moins une Relique en votre possession
  - 1 point par Relique en votre possession de plus que votre adversaire (max 3)"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque archéologue"),
        elements_md: r#"### Excavateur S3 (x3)
- Activable (spécialiste, bonus : contrôle un archéologue)
- **Réussite :** Le joueur prend un pion Relique hors table (max une fois par tour et par excavateur)

### Archéologues Civil (x4)

### Reliques
Prise (1)"#,
        special_rules_md: r#"### Excavation
Au début du tour de chaque joueur, pour chaque pion Relique mis de côté, ce joueur doit poser une relique au contact de l’excavateur dont elle provient. Un combattant au contact peut directement en prendre le contrôle."#,
        common_rule_slugs: &["civils", "specialistes", "activable", "prise"],
    },
    ScenarioSeed {
        slug: "ia-fantome",
        name: "I.A. Fantôme",
        map_filename: Some("IAFantome.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :**
  - 1 point si vous avez détruit strictement plus de Serveurs que votre adversaire
  - 1 point chacun si tous les Serveurs ont été détruits
- **Fin de partie :**
  - 1 point par Serveur détruit
  - 1 point si vous avez détruit l’IA Fantôme"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque console"),
        elements_md: r#"### IA Fantôme
Chaque joueur choisit secrètement l’un de ses Serveurs. Si ce serveur est détruit, l’IA l’est aussi. Au début du tour, si l’IA est dans un serveur examiné, elle peut se déplacer vers un serveur non-examiné non détruit.

### Console S3 (x3)
- Activable (spécialiste, bonus : hacker)
- **Réussite :** désigne un Serveur adverse examiné ; l’adversaire indique si son IA y est ; cette console ne peut plus être activée

### Serveur S1 (3 par joueur)
Placés après le jet de lieutenant. Une fois examinés : Destructible STR 2, ARM 4."#,
        special_rules_md: r#"### Master breacher
À la fin du déploiement de sa réserve, un joueur désigne un combattant déployé non-token, non REM, non VEH et non-TAG comme master breacher (charges creuses + Specialist Operative)."#,
        common_rule_slugs: &["specialistes", "activable", "destructible"],
    },
    ScenarioSeed {
        slug: "largage-aerien",
        name: "Largage aérien",
        map_filename: Some("LargageAerien.png"),
        flavor_text: Some(
            "La livraison est pour ce soir. Espérons que personne n’ait intercepté cette communication…",
        ),
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de partie :**
  - 1 point par Colis contrôlé
  - 1 point si vous contrôlez strictement plus de Balises que votre adversaire"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque Balise"),
        elements_md: r#"### Balises S3 (x6)
- Contrôle (contact)
- Activable (spécialiste, bonus : forward observer)
- **Réussite :** Le joueur prend un pion Colis hors table ; cette balise ne peut plus être activée

### Colis (x6)
Apparaît via Largage Aérien. Prise (1)."#,
        special_rules_md: r#"### Largage aérien
Au début du tour de chaque joueur, pour chaque pion Colis mis de côté, ce joueur pose ce Colis au contact de la Balise dont il provient. Un combattant au contact peut directement en prendre le contrôle."#,
        common_rule_slugs: &["specialistes", "activable", "controle", "prise"],
    },
    ScenarioSeed {
        slug: "le-combat-de-lesprit",
        name: "Le Combat de l’Esprit",
        map_filename: Some("LeCombatDeLEsprit.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Spécial :** il y a 9 points d’Objectifs Secondaires sur ce scénario
- **Fin de partie :** 1 point si vous avez terminé strictement plus d’Objectifs Secondaires que votre adversaire (un secondaire est terminé si un joueur en a marqué les 3 points)"#,
        deployment_notes_md: Some(
            r#"Un seul deck d’Objectifs Secondaires est utilisé. Chaque joueur a 3 Objectifs Secondaires. Tirage A/B : A choisit 1 et le retire ; B choisit 1 et le retire ; A prend 1 ; B prend 2 ; A prend 2 ; B prend 1."#,
        ),
        exclusion_zones_md: None,
        elements_md: "",
        special_rules_md: "",
        common_rule_slugs: &[],
    },
    ScenarioSeed {
        slug: "razzia",
        name: "Razzia",
        map_filename: Some("Razzia.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de partie :**
  - 1 point si vous portez au moins 1 Colis
  - 1 point par Colis porté de plus que votre adversaire (maximum 3)
  - 1 point si vous avez récupéré strictement plus de Codes que votre adversaire
  - 1 point par Distributeur contrôlé"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque console et distributeur"),
        elements_md: r#"### Consoles S3 (x2)
Si un joueur a moins de deux Codes : Activable (spécialiste, bonus : le joueur n’a pas de Code) — reçoit un Code (max 2 par Console et par tour).

### Codes
Un joueur ne peut pas avoir plus de deux Codes.

### Distributeurs S3 (x2)
- Contrôle (contact)
- Activable (spécialiste si le joueur a un Code, bonus : le joueur a 2 Codes)
- **Réussite :** récupère un Colis (max 1 par Distributeur et par tour) et défausse un Code

### Colis
Prise (1)"#,
        special_rules_md: "",
        common_rule_slugs: &["specialistes", "activable", "controle", "prise"],
    },
    ScenarioSeed {
        slug: "saboter-les-defenses",
        name: "Saboter les défenses",
        map_filename: Some("SaboterLesDefenses.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de partie :**
  - 1 point (max. 3) par Frappe orbitale réussie de plus que l’adversaire
  - 1 point par Système de Défense adverse détruit
  - 1 point si vous avez contrôlé plus de Consoles que votre adversaire"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque console"),
        elements_md: r#"### Système de Défense S3 (3 par joueur)
Destructible : STR 2, ARM 4

### Consoles S3 (x4)
- Activable (spécialiste, bonus : le Système de Défense ciblé a au moins une Wound)
- Avant le test, désigner un Système de Défense
- **Réussite :** ce Système est perturbé jusqu’à la fin du tour ; cette Console ne peut plus être activée ce tour-ci
- Contrôle (contact)"#,
        special_rules_md: r#"### Frappes orbitales
À la fin du tour de chaque joueur, le joueur actif réussit une Frappe Orbitale par Système de Défense ennemi détruit ou perturbé.

### Master breacher
À la fin du déploiement de sa réserve, désigner un combattant déployé non-token, non REM, non VEH et non-TAG (charges creuses + Specialist Operative)."#,
        common_rule_slugs: &["specialistes", "activable", "controle", "destructible"],
    },
    ScenarioSeed {
        slug: "scene-de-crime",
        name: "Scène de crime",
        map_filename: Some("SceneDeCrime.png"),
        flavor_text: None,
        end_condition_md: "Fin du troisième round ou fin du tour d’un joueur qui commence son tour en retraite.",
        objectives_md: r#"- **Fin de round :**
  - 1 point si vous avez inspecté strictement plus de Tech-Coffin que votre adversaire
  - 1 point si vous avez interrogé plus ou autant de Témoins que votre adversaire (min. 1)
- **Fin de partie :** 1 point si vous avez éliminé le Coupable de votre adversaire"#,
        deployment_notes_md: None,
        exclusion_zones_md: Some("à 4 pouces de chaque tech-coffin"),
        elements_md: r#"Il n’est pas possible d’interagir avec ses témoins ou son coupable, uniquement avec ceux déployés par l’adversaire.

### Tech-Coffins S3 (x3)
- Activable (spécialiste, bonus : docteur ou paramédic)
- **Réussite :** l’adversaire déploie un Témoin à moins de 8” ; le joueur a inspecté ce Tech-coffin

### Témoins Civil (3 par joueur)
Quand un Témoin est Rallié, il est interrogé et retiré. Au premier interrogatoire, l’adversaire déploie son Coupable hors de sa ZD.

### Coupable CSU (1 par joueur)
Ne génère ni ne peut utiliser d’ordre. Peut être déployé allongé."#,
        special_rules_md: "",
        common_rule_slugs: &["civils", "specialistes", "activable"],
    },
];
