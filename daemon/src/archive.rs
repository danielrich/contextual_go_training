//! Streaming JSONL ingestion. Raw records are retained independently of training eligibility.
use crate::{
    go,
    model::{Game, Result},
};
use serde_json::Value;

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn rank(player: &Value) -> String {
    let value = player.get("rank").or_else(|| player.get("ranking"));
    if let Some(s) = value.and_then(Value::as_str) {
        return s.into();
    }
    match value
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && *n >= 0. && *n <= 50.)
    {
        Some(n) if n < 30. => format!("{}k", 30 - n.floor() as i64),
        Some(n) => format!("{}d", n.floor() as i64 - 29),
        _ => "Unknown".into(),
    }
}
pub fn normalize_ogs(raw: &Value) -> Result<Game> {
    let id = raw
        .get("game_id")
        .or_else(|| raw.get("id"))
        .and_then(Value::as_u64)
        .ok_or("Missing game ID")?;
    let size = raw["width"].as_u64().ok_or("Missing width")? as usize;
    if ![9, 13, 19].contains(&size) || raw["height"].as_u64() != Some(size as u64) {
        return Err("Board not supported for practice".into());
    }
    let komi = raw["komi"]
        .as_f64()
        .filter(|v| v.is_finite() && v.abs() <= 400.)
        .ok_or("Missing or invalid komi")?;
    let rules =
        normalize_rules(&text(raw, "rules")).ok_or("Rules need normalization before grading")?;
    let handicap = raw["handicap"].as_u64().unwrap_or(0) as usize;
    let initial_player = match text(raw, "initial_player").as_str() {
        "white" | "W" => "W",
        "black" | "B" => "B",
        _ => {
            if handicap > 0 {
                "W"
            } else {
                "B"
            }
        }
    }
    .to_string();
    let mut setup = vec![];
    for (key, color) in [("black", "B"), ("white", "W")] {
        let encoded = text(&raw["initial_state"], key);
        let bytes = encoded.as_bytes();
        if bytes.len() % 2 != 0 {
            return Err("Invalid initial setup".into());
        }
        for pair in bytes.chunks_exact(2) {
            let x = pair[0].checked_sub(b'a').ok_or("Invalid setup point")? as usize;
            let y = pair[1].checked_sub(b'a').ok_or("Invalid setup point")? as usize;
            if x >= size || y >= size {
                return Err("Setup outside board".into());
            }
            setup.push([color.to_string(), go::vertex(x, y, size)]);
        }
    }
    let mut moves = vec![];
    let mut color = initial_player.clone();
    for m in raw["moves"].as_array().ok_or("Missing moves")? {
        let x = m[0].as_i64().ok_or("Invalid move coordinate")?;
        let y = m[1].as_i64().ok_or("Invalid move coordinate")?;
        let vertex = if x == -1 && y == -1 {
            "pass".into()
        } else if x >= 0 && y >= 0 && (x as usize) < size && (y as usize) < size {
            go::vertex(x as usize, y as usize, size)
        } else {
            return Err("Move outside board".into());
        };
        moves.push([color.clone(), vertex]);
        color = if color == "B" { "W".into() } else { "B".into() };
    }
    let players = &raw["players"];
    Ok(Game{id:format!("ogs-{id}"),size,komi,rules,moves,setup,initial_player,
        black:text(&players["black"],"username"),white:text(&players["white"],"username"),
        black_rank:rank(&players["black"]),white_rank:rank(&players["white"]),
        source:format!("OGS archive game {id}"),source_url:format!("https://online-go.com/game/{id}"),
        date:raw.get("start_time").map(Value::to_string).unwrap_or_default(),speed:text(&raw["time_control"],"speed"),handicap,sample:false,
        rank_provenance:"OGS archive player-rank snapshot; historical rank at game time is not independently verified".into()})
}

#[derive(Debug, serde::Serialize)]
pub struct ImportReport {
    pub source: String,
    pub previous_line: u64,
    pub last_line: u64,
    pub read: u64,
    pub normalized: u64,
    pub retained_with_issue: u64,
}

pub fn import_json(db: &crate::db::Db, path: &std::path::Path, limit: u64) -> Result<ImportReport> {
    use sha2::{Digest, Sha256};
    use std::io::{BufRead, BufReader};
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let meta = path.metadata().map_err(|e| e.to_string())?;
    let source = format!(
        "{}:{}:{:?}",
        path.display(),
        meta.len(),
        meta.modified().map_err(|e| e.to_string())?
    );
    let previous = db.import_cursor(&source)?;
    let f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader: Box<dyn std::io::Read> = if path.extension().and_then(|s| s.to_str()) == Some("gz")
    {
        Box::new(flate2::read::MultiGzDecoder::new(f))
    } else {
        Box::new(f)
    };
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut number = 0u64;
    let mut report = ImportReport {
        source: source.clone(),
        previous_line: previous,
        last_line: previous,
        read: 0,
        normalized: 0,
        retained_with_issue: 0,
    };
    let mut batch = Vec::with_capacity(250);
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| format!("Archive read error at line {}: {e}", number + 1))?;
        if n == 0 {
            break;
        }
        number += 1;
        if number <= previous {
            continue;
        }
        let raw = line.trim_end_matches(['\n', '\r']).to_string();
        let parsed = serde_json::from_str::<Value>(&raw);
        let (id, game, status) = match parsed {
            Ok(v) => {
                // Raw snapshots have independent identities. The normalized
                // training index still deduplicates by original OGS game ID.
                let id = format!("raw-{:x}", Sha256::digest(raw.as_bytes()));
                match normalize_ogs(&v) {
                    Ok(g) => (id, Some(g), "normalized".to_string()),
                    Err(e) => (id, None, e),
                }
            }
            Err(e) => (
                format!("invalid-{:x}", Sha256::digest(raw.as_bytes())),
                None,
                format!("Invalid JSON: {e}"),
            ),
        };
        report.read += 1;
        if game.is_some() {
            report.normalized += 1;
        } else {
            report.retained_with_issue += 1;
        }
        report.last_line = number;
        batch.push((id, raw, game, status));
        if batch.len() >= 250 {
            db.import_batch(&source, &batch, number)?;
            batch.clear();
            eprintln!(
                "Read through line {number}; {} records this invocation",
                report.read
            );
        }
        if limit > 0 && report.read >= limit {
            break;
        }
    }
    if !batch.is_empty() {
        db.import_batch(&source, &batch, report.last_line)?;
    }
    Ok(report)
}

pub fn normalize_rules(input: &str) -> Option<String> {
    match input.trim().to_lowercase().as_str() {
        "japanese" => Some("japanese".into()),
        "korean" => Some("korean".into()),
        "chinese" => Some("chinese".into()),
        "aga" => Some("aga".into()),
        "nz" | "new-zealand" => Some("new-zealand".into()),
        "tromp-taylor" => Some("tromp-taylor".into()),
        _ => None,
    }
}
