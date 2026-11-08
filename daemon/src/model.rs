use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, String>;
pub type Board = Vec<Vec<u8>>;
fn black() -> String {
    "B".into()
}
fn unknown() -> String {
    "Unknown".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub size: usize,
    pub komi: f64,
    pub rules: String,
    pub moves: Vec<[String; 2]>,
    #[serde(default)]
    pub setup: Vec<[String; 2]>,
    #[serde(default = "black")]
    pub initial_player: String,
    #[serde(default)]
    pub black: String,
    #[serde(default)]
    pub white: String,
    #[serde(default = "unknown")]
    pub black_rank: String,
    #[serde(default = "unknown")]
    pub white_rank: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub speed: String,
    #[serde(default)]
    pub handicap: usize,
    #[serde(default)]
    pub sample: bool,
    #[serde(default)]
    pub rank_provenance: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: usize,
    pub color: String,
    pub anchor: String,
    pub stones: Vec<[usize; 2]>,
    pub liberties: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveEval {
    #[serde(rename = "move")]
    pub vertex: String,
    pub score: f64,
    pub visits: u64,
    #[serde(default)]
    pub pv: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub score: f64,
    pub ownership: Vec<f64>,
    pub model: String,
    pub visits: u64,
    pub grader_version: String,
    pub candidates: BTreeMap<String, MoveEval>,
    pub suggestions: Vec<MoveEval>,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Problem {
    pub id: String,
    pub size: usize,
    pub board: Board,
    pub turn: usize,
    pub to_play: String,
    pub komi: f64,
    pub rules: String,
    pub source: String,
    pub source_url: String,
    pub source_game_id: String,
    pub black: String,
    pub white: String,
    pub black_rank: String,
    pub white_rank: String,
    pub rank_provenance: String,
    pub date: String,
    pub speed: String,
    pub handicap: usize,
    pub sample: bool,
    pub last_move: Option<String>,
    pub groups: Vec<Group>,
    pub legal_moves: Vec<String>,
    pub analysis: Analysis,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredProblem {
    pub problem: Problem,
    pub game: Game,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub id: String,
    pub problem_id: String,
    pub stage: String,
    pub answer: serde_json::Value,
    #[serde(default)]
    pub reasoning: String,
    pub created: u64,
    pub grader_version: String,
    #[serde(default)]
    pub review: bool,
    #[serde(default)]
    pub session_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackJob {
    pub id: String,
    pub requested: usize,
    pub size: String,
    pub speed: String,
    pub state: String,
    pub completed: usize,
    pub error: Option<String>,
    pub current_moves_done: usize,
    pub current_moves_total: usize,
}

pub fn new_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn mean_rank(black: &str, white: &str) -> String {
    fn index(value: &str) -> Option<u32> {
        let value = value.trim().to_lowercase();
        let suffix = value.chars().last()?;
        if suffix != 'k' && suffix != 'd' {
            return None;
        }
        let n = value[..value.len() - 1].parse::<u32>().ok()?;
        match suffix {
            'k' if (1..=30).contains(&n) => Some(30 - n),
            'd' if (1..=9).contains(&n) => Some(29 + n),
            _ => None,
        }
    }
    match (index(black), index(white)) {
        (Some(b), Some(w)) => {
            let n = (b + w + 1) / 2;
            if n <= 29 {
                format!("{}k", 30 - n)
            } else {
                format!("{}d", n - 29)
            }
        }
        _ => "Unknown".into(),
    }
}
