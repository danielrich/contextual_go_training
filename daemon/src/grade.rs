use crate::model::{Problem, Result};
use serde_json::{json, Value};
pub const VERSION: &str = "whole-board-grader-v2";
pub const STAGES: [&str; 4] = ["winner", "score", "groups", "candidates"];
pub fn candidate_loss(baseline: f64, candidate: f64, player: &str) -> f64 {
    ((baseline - candidate) * if player == "B" { 1. } else { -1. }).max(0.)
}
fn round(v: f64) -> f64 {
    (v * 10.).round() / 10.
}
pub fn group_status(p: &Problem, group: &crate::model::Group) -> &'static str {
    let values: Vec<_> = group
        .stones
        .iter()
        .map(|[x, y]| {
            p.analysis.ownership[y * p.size + x] * if group.color == "B" { 1. } else { -1. }
        })
        .collect();
    if values.iter().all(|v| *v >= 0.9) {
        "secure-looking"
    } else if values.iter().all(|v| *v <= -0.7) {
        "at-risk"
    } else {
        "uncertain"
    }
}
pub fn grade(p: &Problem, stage: &str, answer: &Value) -> Result<Value> {
    grade_with_version(p, stage, answer, VERSION)
}
pub fn grade_with_version(
    p: &Problem,
    stage: &str,
    answer: &Value,
    version: &str,
) -> Result<Value> {
    if ![VERSION, "whole-board-grader-v1"].contains(&version) {
        return Err("Unknown grading version".into());
    }
    let legacy = version == "whole-board-grader-v1";
    let index = STAGES
        .iter()
        .position(|s| *s == stage)
        .ok_or("Unknown question")?;
    let mut result = json!({"stage":stage,"next_stage":STAGES.get(index+1),"pending":false,"grader_version":version});
    match stage {
        "winner" => {
            let a = answer
                .as_str()
                .filter(|s| ["B", "W", "even"].contains(s))
                .ok_or("Choose Black, White or close")?;
            let expected = if p.analysis.score.abs() <= 2. {
                "even"
            } else if p.analysis.score > 0. {
                "B"
            } else {
                "W"
            };
            result["correct"] = json!(a == expected);
            result["expected"] = json!(expected);
            result["message"]=json!("Close means within 2 estimated points. Commit to your count before seeing the score.");
        }
        "score" => {
            let a = answer
                .as_f64()
                .filter(|v| v.is_finite() && v.abs() <= (p.size * p.size + 400) as f64)
                .ok_or("Enter a finite score estimate")?;
            result["correct"] = json!((a - p.analysis.score).abs() <= 3.);
            result["score"] = json!(round(p.analysis.score));
            result["error"] = json!(round((a - p.analysis.score).abs()));
            result["message"]=json!("KataGo estimates the final lead with strong play, including komi. Within 3 points earns credit; this is not simply settled territory.");
        }
        "groups" => {
            let a = answer.as_object().ok_or("Assess each marked group")?;
            let mut groups = vec![];
            for g in &p.groups {
                let value = a
                    .get(&g.id.to_string())
                    .and_then(Value::as_str)
                    .filter(|v| ["secure-looking", "at-risk", "uncertain"].contains(v))
                    .ok_or("Assess each marked group")?;
                let expected = group_status(p, g);
                groups.push(json!({"id":g.id,"anchor":g.anchor,"expected":expected,"answer":value,"correct":value==expected}));
            }
            result["correct"] = json!(groups.iter().all(|g| g["correct"] == true));
            result["groups"] = json!(groups);
            result["message"]=json!("Ownership agreement only: secure-looking requires at least 90% own-color ownership at every stone; at-risk requires at least 70% opponent ownership at every stone. Otherwise uncertain. These estimates do not prove life, death or seki.");
        }
        "candidates" => {
            let required = p.legal_moves.len().min(3);
            let moves = answer
                .as_array()
                .filter(|a| a.len() == required && required > 0)
                .ok_or("Choose the requested number of different legal candidates")?;
            let mut seen = std::collections::HashSet::new();
            let mut evaluated = vec![];
            let mut missing = vec![];
            for m in moves {
                let m = m.as_str().ok_or("Invalid candidate")?;
                if !seen.insert(m) || !p.legal_moves.iter().any(|v| v == m) {
                    return Err(
                        "Choose three different legal candidates, including pass if useful".into(),
                    );
                }
                if let Some(v) = p.analysis.candidates.get(m) {
                    let loss = candidate_loss(p.analysis.score, v.score, &p.to_play);
                    evaluated.push(json!({"move":m,"loss":loss,"correct":if legacy {loss<=2.}else{loss<2.},"blunder":loss>5.,"serious_blunder":loss>10.,"pv":v.pv,"visits":v.visits}));
                } else {
                    missing.push(m.to_string());
                }
            }
            let top_hit = p
                .analysis
                .suggestions
                .first()
                .map(|top| seen.contains(top.vertex.as_str()))
                .unwrap_or(false);
            result["engine_top_hit"] = json!(top_hit);
            if !missing.is_empty() {
                result["pending"] = json!(true);
                result["missing"] = json!(missing);
                result["correct"] = Value::Null;
            } else {
                result["correct"] =
                    json!(evaluated.iter().all(|v| v["correct"] == true) || (!legacy && top_hit));
            }
            result["candidates"] = json!(evaluated);
            result["suggestions"] = json!(p.analysis.suggestions);
            result["message"]=json!("Each candidate is independently searched from this position. A set earns credit when every move loses less than 2 estimated points, or it includes the engine top move. Blunder: more than 5 points lost; serious blunder: more than 10. Search noise can affect close calls; unevaluated moves remain pending.");
        }
        _ => unreachable!(),
    }
    Ok(result)
}
