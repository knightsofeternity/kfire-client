//! Reading what KFIRE Guilde saved (`KFireGuildeDB`), game closed, and turning
//! it into the `guild` object of the Forever `match_result`. The contract is
//! `wow-addon/docs/kfire-guilde-liaison.md`.
//!
//! Requests and quests stay in the game; the roster snapshot, the members'
//! profiles, the professions with their recipes and the hourly presence go up.

use super::savedvars::{self, Value};
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;
use std::path::Path;

pub const SAVED_FILE: &str = "KFireGuilde.lua";

const MAX_ROSTER: usize = 1000;
const MAX_MEMBERS: usize = 1000;
const MAX_PROFESSIONS: usize = 3000;
const MAX_RECIPES: usize = 1000;
const MAX_ITEM_NAMES: usize = 20_000;
const MAX_DAYS: usize = 70;

fn table(v: Option<&Value>) -> Option<&BTreeMap<String, Value>> {
    match v? {
        Value::Table(t) => Some(t),
        _ => None,
    }
}

/// The addon turns every shared value into a string; older records may hold numbers.
fn number(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::Num(n) => Some(*n as i64),
        Value::Str(s) => s.trim().parse::<f64>().ok().map(|n| n as i64),
        _ => None,
    }
}

fn rfc3339(secs: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(secs, 0).map(|t| t.to_rfc3339())
}

/// « Nom-Royaume », as the game writes it: 2 to 24 characters for the name.
fn character_ok(name: &str) -> bool {
    match name.split_once('-') {
        Some((n, r)) => {
            let (n, r) = (n.chars().count(), r.chars().count());
            (2..=24).contains(&n) && (1..=64).contains(&r)
        }
        None => false,
    }
}

fn text(v: &Value, key: &str, max: usize) -> Option<String> {
    v.str(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().count() <= max && !s.contains(['\n', '\r', '\0']))
}

/// The `guild` object of one saved file, or None when the addon never saw a guild.
pub fn from_saved(src: &str) -> Option<(i64, Json)> {
    let root = savedvars::global(src, "KFireGuildeDB")?;
    if number(&root, "v")? != 1 {
        return None;
    }
    let roster = root.get("roster")?;
    let snapshot = number(roster, "date")?;
    let guild_name = text(roster, "guilde", 64)?;
    let realm_norm = text(roster, "royaume", 64)?;
    let stores = root.get("magasins");

    let mut roster_rows = Vec::new();
    for (name, m) in table(roster.get("membres")).into_iter().flatten() {
        if !character_ok(name) || roster_rows.len() >= MAX_ROSTER {
            continue;
        }
        roster_rows.push(json!({
            "name": name,
            "level": number(m, "niveau"),
            "class": text(m, "classe", 32),
            "rank": text(m, "rang", 64),
            "rank_index": number(m, "rangIndex"),
            "last_seen_at": number(m, "vu").and_then(rfc3339),
        }));
    }

    let mut members = Vec::new();
    for (name, f) in table(stores.and_then(|s| s.get("membres")))
        .into_iter()
        .flatten()
    {
        let Some(at) = number(f, "_maj").and_then(rfc3339) else {
            continue;
        };
        if !character_ok(name) || members.len() >= MAX_MEMBERS {
            continue;
        }
        let role = f
            .str("role")
            .filter(|r| matches!(r.as_str(), "T" | "H" | "D"));
        members.push(json!({
            "name": name,
            "pseudo": text(f, "pseudo", 24),
            "main": f.str("main").filter(|m| character_ok(m)),
            "role": role,
            "addon_version": text(f, "version", 16),
            "updated_at": at,
        }));
    }

    let names = table(root.get("noms"));
    let mut item_names = Map::new();
    let mut professions = Vec::new();
    for (key, f) in table(stores.and_then(|s| s.get("metiers")))
        .into_iter()
        .flatten()
    {
        let Some((name, profession)) = key.split_once('/') else {
            continue;
        };
        let Some(at) = number(f, "_maj").and_then(rfc3339) else {
            continue;
        };
        let profession = text(f, "metier", 64).unwrap_or_else(|| profession.to_string());
        if !character_ok(name) || professions.len() >= MAX_PROFESSIONS {
            continue;
        }
        let recipes: Vec<String> = f
            .str("recettes")
            .unwrap_or_default()
            .split(',')
            .filter(|r| {
                let digits = r.strip_prefix('s').unwrap_or(r);
                !digits.is_empty()
                    && digits.len() <= 9
                    && digits.bytes().all(|b| b.is_ascii_digit())
            })
            .take(MAX_RECIPES)
            .map(str::to_string)
            .collect();
        for r in &recipes {
            if item_names.len() < MAX_ITEM_NAMES && !item_names.contains_key(r) {
                if let Some(Value::Str(n)) = names.and_then(|t| t.get(r)) {
                    item_names.insert(r.clone(), json!(n));
                }
            }
        }
        professions.push(json!({
            "name": name,
            "profession": profession,
            "level": number(f, "niveau"),
            "max": number(f, "max"),
            "recipes": recipes,
            "updated_at": at,
        }));
    }

    let mut presence: Vec<(String, Json)> = Vec::new();
    for (day, f) in table(stores.and_then(|s| s.get("presence")))
        .into_iter()
        .flatten()
    {
        if day.len() != 8 || !day.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let hours: Vec<Json> = f
            .str("h")
            .unwrap_or_default()
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (0..=1000).contains(n))
                    .map_or(Json::Null, Json::from)
            })
            .collect();
        if hours.len() != 24 {
            continue;
        }
        let iso = format!("{}-{}-{}", &day[0..4], &day[4..6], &day[6..8]);
        presence.push((iso.clone(), json!({ "day": iso, "hours": hours })));
    }
    presence.sort_by(|a, b| b.0.cmp(&a.0));
    presence.truncate(MAX_DAYS);

    let me = root.get("moi");
    Some((
        snapshot,
        json!({
            "addon_version": text(&root, "version", 16),
            "guild_name": guild_name,
            "realm_norm": realm_norm,
            "snapshot_at": rfc3339(snapshot),
            "me": {
                "pseudo": me.and_then(|m| text(m, "pseudo", 24)),
                "main": me.and_then(|m| m.str("main")).filter(|m| character_ok(m)),
            },
            "roster": roster_rows,
            "members": members,
            "professions": professions,
            "item_names": item_names,
            "presence": presence.into_iter().map(|(_, p)| p).collect::<Vec<_>>(),
        }),
    ))
}

/// The newest guild object across every game account of these editions.
pub fn read(editions: &[&Path]) -> Option<Json> {
    let mut best: Option<(i64, Json)> = None;
    for ed in editions {
        let Ok(accounts) = std::fs::read_dir(ed.join("WTF").join("Account")) else {
            continue;
        };
        for acc in accounts.flatten() {
            let file = acc.path().join("SavedVariables").join(SAVED_FILE);
            let Ok(src) = std::fs::read_to_string(&file) else {
                continue;
            };
            match from_saved(&src) {
                Some((at, g)) if best.as_ref().is_none_or(|(b, _)| at > *b) => best = Some((at, g)),
                Some(_) => {}
                None => log::info!("wow: no guild in {}", file.display()),
            }
        }
    }
    best.map(|(_, g)| g)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Shaped like the file WoW writes for KFireGuildeDB (bracketed keys, trailing commas).
    pub(crate) const SAVED: &str = r#"
KFireGuildeDB = {
["v"] = 1,
["version"] = "0.2.0",
["moi"] = { ["pseudo"] = "Oura", ["main"] = "Ouranos-Chogall", ["roles"] = { ["Ouranos-Chogall"] = "D", }, },
["noms"] = { ["2996"] = "Rouleau d'étoffe de lin", ["4343"] = "Pantalon en lin brun", ["9999"] = "Autre", },
["roster"] = {
  ["date"] = 1763600000, ["guilde"] = "Knights of Eternity", ["royaume"] = "Chogall",
  ["membres"] = {
    ["Ouranos-Chogall"] = { ["niveau"] = 60, ["classe"] = "MAGE", ["rang"] = "Officier", ["rangIndex"] = 1, ["vu"] = 1763600000, },
    ["Sansaddon-Chogall"] = { ["niveau"] = 12, ["classe"] = "MAGE", ["rang"] = "Recrue", ["rangIndex"] = 4, },
    ["Pasunnom"] = { ["niveau"] = 1, },
  },
},
["magasins"] = {
  ["membres"] = {
    ["Ouranos-Chogall"] = { ["_maj"] = 1763590000, ["_auteur"] = "Ouranos-Chogall", ["pseudo"] = "Oura", ["main"] = "Ouranos-Chogall", ["role"] = "D", ["version"] = "0.2.0", },
    ["Malkai-Chogall"] = { ["_maj"] = 1763580000, ["pseudo"] = "Malk", ["role"] = "X", },
  },
  ["metiers"] = {
    ["Malkai-Chogall/Couture"] = { ["_maj"] = 1763580000, ["metier"] = "Couture", ["niveau"] = "75", ["max"] = "150", ["recettes"] = "2996,4343,s7418,abc", },
  },
  ["presence"] = {
    ["20261120"] = { ["_maj"] = 1763600000, ["h"] = "3,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,-,12,-,-", },
    ["2026112"] = { ["h"] = "1", },
  },
  ["demandes"] = { ["Ouranos-Chogall/1"] = { ["_maj"] = 1763600000, ["nom"] = "x", }, },
},
}
"#;

    #[test]
    fn the_saved_file_becomes_the_guild_object() {
        let (at, g) = from_saved(SAVED).unwrap();
        assert_eq!(at, 1763600000);
        assert_eq!(g["guild_name"], "Knights of Eternity");
        assert_eq!(g["realm_norm"], "Chogall");
        assert_eq!(g["addon_version"], "0.2.0");
        assert_eq!(g["me"]["pseudo"], "Oura");
        let roster = g["roster"].as_array().unwrap();
        assert_eq!(roster.len(), 2, "a name without realm is dropped");
        let o = roster
            .iter()
            .find(|r| r["name"] == "Ouranos-Chogall")
            .unwrap();
        assert_eq!(
            (
                o["level"].as_i64(),
                o["class"].as_str(),
                o["rank_index"].as_i64()
            ),
            (Some(60), Some("MAGE"), Some(1))
        );
        let s = roster
            .iter()
            .find(|r| r["name"] == "Sansaddon-Chogall")
            .unwrap();
        assert!(s["last_seen_at"].is_null());
        let members = g["members"].as_array().unwrap();
        let m = members
            .iter()
            .find(|m| m["name"] == "Malkai-Chogall")
            .unwrap();
        assert!(m["role"].is_null(), "an unknown role becomes null");
        let p = &g["professions"][0];
        assert_eq!(
            (p["level"].as_i64(), p["max"].as_i64()),
            (Some(75), Some(150))
        );
        assert_eq!(p["recipes"], json!(["2996", "4343", "s7418"]));
        assert_eq!(
            g["item_names"],
            json!({ "2996": "Rouleau d'étoffe de lin", "4343": "Pantalon en lin brun" })
        );
        let days = g["presence"].as_array().unwrap();
        assert_eq!(days.len(), 1, "a malformed day is dropped");
        assert_eq!(days[0]["day"], "2026-11-20");
        assert_eq!(days[0]["hours"][0], 3);
        assert!(days[0]["hours"][1].is_null());
        assert_eq!(days[0]["hours"][21], 12);
        assert!(
            g.get("demandes").is_none() && g.to_string().find("demandes").is_none(),
            "requests stay in game"
        );
    }

    #[test]
    fn no_guild_no_object() {
        assert!(from_saved("KFireGuildeDB = { [\"v\"] = 1, }").is_none());
        assert!(from_saved("KFireGuildeDB = { [\"v\"] = 2, [\"roster\"] = {}, }").is_none());
        assert!(from_saved("garbage {{").is_none());
    }

    #[test]
    fn the_newest_account_wins() {
        let ed = std::env::temp_dir().join(format!("kfire-wow-guild-{}", uuid::Uuid::new_v4()));
        for (acc, date) in [("111#1", 1763600000), ("222#1", 1763500000)] {
            let dir = ed
                .join("WTF")
                .join("Account")
                .join(acc)
                .join("SavedVariables");
            std::fs::create_dir_all(&dir).unwrap();
            let src = SAVED.replace("[\"date\"] = 1763600000", &format!("[\"date\"] = {date}"));
            std::fs::write(dir.join(SAVED_FILE), src).unwrap();
        }
        let g = read(&[ed.as_path()]).unwrap();
        assert_eq!(g["snapshot_at"], rfc3339(1763600000).unwrap());
        std::fs::remove_dir_all(ed).unwrap();
    }
}
