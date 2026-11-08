//! Position reconstruction. KataGo's policy supplies the authoritative legal mask.
use crate::model::{Board, Game, Group, Result};
use std::collections::{BTreeSet, HashSet};
const LETTERS: &[u8] = b"ABCDEFGHJKLMNOPQRST";

pub fn vertex(x: usize, y: usize, size: usize) -> String {
    format!("{}{}", LETTERS[x] as char, size - y)
}
pub fn coord(v: &str, size: usize) -> Result<(usize, usize)> {
    let bytes = v.as_bytes();
    if bytes.len() < 2 || !bytes[1..].iter().all(u8::is_ascii_digit) {
        return Err("Invalid board coordinate".into());
    }
    let x = LETTERS
        .iter()
        .position(|b| *b == bytes[0])
        .ok_or("Invalid board column")?;
    let row = v[1..].parse::<usize>().map_err(|_| "Invalid board row")?;
    if x >= size || row == 0 || row > size {
        return Err("Move is outside the board".into());
    }
    Ok((x, size - row))
}
fn neighbors(x: usize, y: usize, n: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(4);
    if x > 0 {
        result.push((x - 1, y));
    }
    if y > 0 {
        result.push((x, y - 1));
    }
    if x + 1 < n {
        result.push((x + 1, y));
    }
    if y + 1 < n {
        result.push((x, y + 1));
    }
    result
}
pub fn group_at(
    board: &Board,
    x: usize,
    y: usize,
) -> (BTreeSet<(usize, usize)>, HashSet<(usize, usize)>) {
    let color = board[y][x];
    let mut stones = BTreeSet::new();
    let mut liberties = HashSet::new();
    if color == 0 {
        return (stones, liberties);
    }
    let mut stack = vec![(x, y)];
    while let Some((a, b)) = stack.pop() {
        if !stones.insert((a, b)) {
            continue;
        }
        for (c, d) in neighbors(a, b, board.len()) {
            if board[d][c] == 0 {
                liberties.insert((c, d));
            } else if board[d][c] == color && !stones.contains(&(c, d)) {
                stack.push((c, d));
            }
        }
    }
    (stones, liberties)
}
pub fn play(board: &Board, color: &str, v: &str, suicide: bool) -> Result<Board> {
    if color != "B" && color != "W" {
        return Err("Unknown player".into());
    }
    let mut result = board.clone();
    if v == "pass" {
        return Ok(result);
    }
    let (x, y) = coord(v, board.len())?;
    if result[y][x] != 0 {
        return Err("Occupied intersection".into());
    }
    let stone = if color == "B" { 1 } else { 2 };
    result[y][x] = stone;
    for (a, b) in neighbors(x, y, board.len()) {
        if result[b][a] == 3 - stone {
            let (group, libs) = group_at(&result, a, b);
            if libs.is_empty() {
                for (c, d) in group {
                    result[d][c] = 0;
                }
            }
        }
    }
    let (group, libs) = group_at(&result, x, y);
    if libs.is_empty() {
        if !suicide {
            return Err("Illegal suicide".into());
        }
        for (a, b) in group {
            result[b][a] = 0;
        }
    }
    Ok(result)
}
pub fn to_play(g: &Game, turn: usize) -> String {
    if turn == 0 {
        g.initial_player.clone()
    } else if g.moves[turn - 1][0] == "B" {
        "W".into()
    } else {
        "B".into()
    }
}
pub fn last_playable_turn(g: &Game) -> usize {
    for i in 1..g.moves.len() {
        if g.moves[i - 1][1] == "pass" && g.moves[i][1] == "pass" {
            return i;
        }
    }
    g.moves.len()
}
pub fn replay(g: &Game, turn: usize) -> Result<Board> {
    if ![9, 13, 19].contains(&g.size) || turn > g.moves.len() {
        return Err("Unsupported board or turn".into());
    }
    let mut b = vec![vec![0; g.size]; g.size];
    for [color, v] in &g.setup {
        let (x, y) = coord(v, g.size)?;
        if b[y][x] != 0 {
            return Err("Overlapping setup stones".into());
        }
        b[y][x] = match color.as_str() {
            "B" => 1,
            "W" => 2,
            _ => return Err("Unknown setup player".into()),
        };
    }
    let suicide = matches!(g.rules.as_str(), "new-zealand" | "tromp-taylor" | "nz");
    let mut expected = g.initial_player.clone();
    for [color, v] in g.moves.iter().take(turn) {
        if color != &expected {
            return Err("Non-alternating move sequence".into());
        }
        b = play(&b, color, v, suicide)?;
        expected = if color == "B" { "W".into() } else { "B".into() };
    }
    Ok(b)
}
pub fn groups(board: &Board) -> Vec<Group> {
    let mut seen = HashSet::new();
    let mut result = vec![];
    let size = board.len();
    for y in 0..size {
        for x in 0..size {
            if board[y][x] == 0 || seen.contains(&(x, y)) {
                continue;
            }
            let (stones, libs) = group_at(board, x, y);
            seen.extend(stones.iter().copied());
            result.push(Group {
                id: result.len() + 1,
                color: if board[y][x] == 1 {
                    "B".into()
                } else {
                    "W".into()
                },
                anchor: vertex(x, y, size),
                stones: stones.into_iter().map(|(a, b)| [a, b]).collect(),
                liberties: libs.len(),
            });
        }
    }
    result
}
