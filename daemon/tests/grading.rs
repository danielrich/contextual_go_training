use serde_json::json;
use whole_board::{grade, model::*};
fn problem() -> Problem {
    serde_json::from_value(json!({"id":"p","size":9,"board":vec![vec![0;9];9],"turn":1,"to_play":"W","komi":6.5,"rules":"japanese","source":"test","source_url":"","source_game_id":"g","black":"a","white":"b","black_rank":"20k","white_rank":"3d","rank_provenance":"fixture","date":"","speed":"live","handicap":0,"sample":false,"last_move":null,"groups":[],"legal_moves":["D4","E5","pass"],"analysis":{"score":-4.,"ownership":vec![0.;81],"model":"m","visits":200,"grader_version":grade::VERSION,"candidates":{"D4":{"move":"D4","score":-4.,"visits":200,"pv":["D4"]},"E5":{"move":"E5","score":-3.,"visits":200,"pv":[]}},"suggestions":[],"complete":false}})).unwrap()
}
#[test]
fn arbitrary_legal_candidates_can_be_pending_but_never_wrong_by_omission() {
    let p = problem();
    let f = grade::grade(&p, "candidates", &json!(["D4", "E5", "pass"])).unwrap();
    assert_eq!(f["pending"], true);
    assert!(f["correct"].is_null());
}
#[test]
fn score_loss_uses_white_perspective() {
    assert_eq!(grade::candidate_loss(-4., -1., "W"), 3.);
    assert_eq!(grade::candidate_loss(4., 1., "B"), 3.);
}
#[test]
fn duplicate_or_illegal_candidates_are_rejected() {
    let p = problem();
    assert!(grade::grade(&p, "candidates", &json!(["D4", "D4", "pass"])).is_err());
    assert!(grade::grade(&p, "candidates", &json!(["A9", "E5", "pass"])).is_err());
}
#[test]
fn score_tolerance_and_winner_match_original_flow() {
    let p = problem();
    assert_eq!(
        grade::grade(&p, "winner", &json!("W")).unwrap()["correct"],
        true
    );
    assert_eq!(
        grade::grade(&p, "score", &json!(-1.)).unwrap()["correct"],
        true
    );
}
#[test]
fn candidate_sets_accept_top_move_and_track_strict_blunder_thresholds() {
    let mut p = problem();
    p.analysis.candidates.insert(
        "pass".into(),
        MoveEval {
            vertex: "pass".into(),
            score: 6.01,
            visits: 200,
            pv: vec![],
        },
    );
    p.analysis.candidates.get_mut("E5").unwrap().score = -2.;
    let f = grade::grade(&p, "candidates", &json!(["D4", "E5", "pass"])).unwrap();
    assert_eq!(f["correct"], false);
    assert_eq!(f["candidates"][1]["correct"], false);
    assert_eq!(f["candidates"][2]["blunder"], true);
    assert_eq!(f["candidates"][2]["serious_blunder"], true);
    p.analysis
        .suggestions
        .push(p.analysis.candidates["D4"].clone());
    let f = grade::grade(&p, "candidates", &json!(["D4", "E5", "pass"])).unwrap();
    assert_eq!(f["correct"], true);
    assert_eq!(f["engine_top_hit"], true);
}

#[test]
fn mean_rank_handles_unicode_unknown_and_kyu_dan_boundary() {
    assert_eq!(mean_rank("初段", "10k"), "Unknown");
    assert_eq!(mean_rank("10k", "7k"), "8k");
    assert_eq!(mean_rank("1k", "1d"), "1d");
}
