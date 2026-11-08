//! SGF mainline import; uploaded originals are retained even if normalization fails.
use crate::{
    archive::normalize_rules,
    go,
    model::{Game, Result},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Node = BTreeMap<String, Vec<String>>;
struct Parser<'a> {
    data: &'a [u8],
    i: usize,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self.i < self.data.len() && self.data[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<String> {
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.data.len() {
            let b = self.data[self.i];
            self.i += 1;
            if b == b']' {
                return String::from_utf8(out).map_err(|_| "SGF must use UTF-8".into());
            }
            if b == b'\\' {
                let next = *self.data.get(self.i).ok_or("Unclosed SGF escape")?;
                self.i += 1;
                if next == b'\r' || next == b'\n' {
                    if next == b'\r' && self.data.get(self.i) == Some(&b'\n') {
                        self.i += 1;
                    }
                    continue;
                }
                out.push(next);
            } else {
                out.push(b);
            }
        }
        Err("Unclosed SGF property".into())
    }
    fn tree(&mut self, depth: usize) -> Result<Vec<Node>> {
        if depth > 64 {
            return Err("SGF variations are too deeply nested".into());
        }
        self.space();
        if self.data.get(self.i) != Some(&b'(') {
            return Err("Expected SGF game tree".into());
        }
        self.i += 1;
        let mut nodes = vec![];
        let mut first_child = None;
        loop {
            self.space();
            match self.data.get(self.i) {
                Some(b';') => {
                    self.i += 1;
                    let mut node = Node::new();
                    loop {
                        self.space();
                        let start = self.i;
                        while self.data.get(self.i).is_some_and(u8::is_ascii_uppercase) {
                            self.i += 1;
                        }
                        if self.i == start {
                            break;
                        }
                        let key = String::from_utf8(self.data[start..self.i].to_vec())
                            .map_err(|e| e.to_string())?;
                        let mut values = vec![];
                        self.space();
                        while self.data.get(self.i) == Some(&b'[') {
                            values.push(self.value()?);
                            self.space();
                        }
                        if values.is_empty() {
                            return Err("SGF property has no value".into());
                        }
                        node.entry(key).or_default().extend(values);
                    }
                    nodes.push(node);
                }
                Some(b'(') => {
                    let branch = self.tree(depth + 1)?;
                    if first_child.is_none() {
                        first_child = Some(branch);
                    }
                }
                Some(b')') => {
                    self.i += 1;
                    if nodes.is_empty() {
                        return Err("Empty SGF game tree".into());
                    }
                    if let Some(child) = first_child {
                        nodes.extend(child);
                    }
                    return Ok(nodes);
                }
                _ => return Err("Malformed or unclosed SGF".into()),
            }
        }
    }
}
pub fn parse(text: &str, source: &str) -> Result<Game> {
    if text.len() > 5_000_000 {
        return Err("SGF must be under 5 MB".into());
    }
    let mut parser = Parser {
        data: text.as_bytes(),
        i: 0,
    };
    let nodes = parser.tree(0)?;
    parser.space();
    if parser.i != parser.data.len() {
        return Err("Import one SGF game per file".into());
    }
    let root = &nodes[0];
    let prop = |key: &str| {
        root.get(key)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default()
    };
    if !prop("GM").is_empty() && prop("GM") != "1" {
        return Err("The SGF is not a Go game".into());
    }
    let size = if prop("SZ").is_empty() {
        19
    } else {
        prop("SZ")
            .parse::<usize>()
            .map_err(|_| "Invalid board size")?
    };
    if ![9, 13, 19].contains(&size) {
        return Err(
            "This board size is retained as an upload but is not supported for practice".into(),
        );
    }
    let komi = prop("KM")
        .parse::<f64>()
        .map_err(|_| "Provide explicit komi before grading this game")?;
    if !komi.is_finite() || komi.abs() > 400. {
        return Err("Invalid komi".into());
    }
    let rules = normalize_rules(&prop("RU")).ok_or("Rules need normalization before grading")?;
    let point = |s: &str| -> Result<String> {
        if s.is_empty() || s == "tt" {
            return Ok("pass".into());
        }
        let b = s.as_bytes();
        if b.len() != 2 || b[0] < b'a' || b[1] < b'a' {
            return Err("Invalid SGF point".into());
        }
        let (x, y) = ((b[0] - b'a') as usize, (b[1] - b'a') as usize);
        if x >= size || y >= size {
            return Err("Move outside board".into());
        }
        Ok(go::vertex(x, y, size))
    };
    let mut setup = vec![];
    for (key, color) in [("AB", "B"), ("AW", "W")] {
        for value in root.get(key).into_iter().flatten() {
            if let Some((a, b)) = value.split_once(':') {
                let (ax, ay) = go::coord(&point(a)?, size)?;
                let (bx, by) = go::coord(&point(b)?, size)?;
                if ax > bx || ay > by {
                    return Err("Invalid setup rectangle".into());
                }
                for y in ay..=by {
                    for x in ax..=bx {
                        setup.push([color.into(), go::vertex(x, y, size)]);
                    }
                }
            } else {
                let v = point(value)?;
                if v == "pass" {
                    return Err("Setup stones cannot pass".into());
                }
                setup.push([color.into(), v]);
            }
        }
    }
    let mut moves = vec![];
    for (i, node) in nodes.iter().enumerate() {
        if i > 0
            && ["AB", "AW", "AE", "PL"]
                .iter()
                .any(|key| node.contains_key(*key))
        {
            return Err("Mid-game setup edits need normalization before grading".into());
        }
        if node.contains_key("B") && node.contains_key("W") {
            return Err("Both players move in the same node".into());
        }
        for color in ["B", "W"] {
            if let Some(v) = node.get(color) {
                if v.len() != 1 {
                    return Err("Move property must contain one point".into());
                }
                moves.push([color.into(), point(&v[0])?]);
            }
        }
    }
    let handicap = if prop("HA").is_empty() {
        0
    } else {
        prop("HA")
            .parse::<usize>()
            .map_err(|_| "Invalid handicap")?
    };
    let initial_player = if prop("PL").is_empty() {
        if handicap > 0 { "W" } else { "B" }.to_string()
    } else {
        prop("PL")
    };
    let speed = prop("GC")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let speed = if ["live", "blitz", "rapid", "correspondence"].contains(&speed.as_str()) {
        speed
    } else {
        let tm = prop("TM").parse::<f64>().unwrap_or(0.);
        if tm > 7200. {
            "correspondence".into()
        } else if tm > 0. {
            "live".into()
        } else {
            "unknown".into()
        }
    };
    let place = prop("PC");
    let source_url = place
        .find("https://online-go.com/game/")
        .map(|i| {
            place[i..]
                .chars()
                .take_while(|c| !c.is_whitespace())
                .collect::<String>()
        })
        .unwrap_or_else(|| prop("SO"));
    let id = source_url
        .strip_prefix("https://online-go.com/game/")
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .map(|s| format!("ogs-{s}"))
        .unwrap_or_else(|| format!("sgf-{:x}", Sha256::digest(text.as_bytes())));
    let g=Game{id,size,komi,rules,moves,setup,initial_player,black:prop("PB"),white:prop("PW"),black_rank:if prop("BR").is_empty(){"Unknown".into()}else{prop("BR")},white_rank:if prop("WR").is_empty(){"Unknown".into()}else{prop("WR")},source:source.into(),source_url,date:prop("DT"),speed,handicap,sample:false,rank_provenance:"Rank values recorded in the uploaded SGF; historical rank at game time is not independently verified".into()};
    go::replay(&g, 0)?;
    Ok(g)
}
