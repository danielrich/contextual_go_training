use serde_json::json;
use whole_board::{archive::normalize_ogs, go, model::Game};

fn game() -> Game {
    serde_json::from_value(json!({"id":"test","size":9,"komi":6.5,"rules":"japanese","moves":[["B","C7"],["W","G3"]],"setup":[],"initial_player":"B","black_rank":"10k","white_rank":"2d","source":"fixture","sample":false})).unwrap()
}

#[test]
fn captures_and_setup_are_replayed_without_mutating_the_game() {
    let mut g = game();
    g.setup = vec![
        ["B".into(), "A8".into()],
        ["B".into(), "B9".into()],
        ["B".into(), "C8".into()],
        ["W".into(), "B8".into()],
    ];
    g.moves = vec![["B".into(), "B7".into()]];
    let b = go::replay(&g, 1).unwrap();
    assert_eq!(b[1][1], 0);
    assert_eq!(b[2][1], 1);
    assert_eq!(g.setup.len(), 4);
}
#[test]
fn late_positions_stop_at_actual_two_pass_boundary() {
    let mut g = game();
    g.moves
        .extend([["B".into(), "pass".into()], ["W".into(), "pass".into()]]);
    assert_eq!(go::last_playable_turn(&g), 3);
}
#[test]
fn group_membership_and_coordinates_are_stable() {
    let g = game();
    let b = go::replay(&g, 2).unwrap();
    let groups = go::groups(&b);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].anchor, "C7");
    assert_eq!(go::coord("J10", 19).unwrap(), (8, 9));
    assert!(go::coord("I10", 19).is_err());
}
#[test]
fn raw_json_preserves_handicap_and_correspondence_without_rank_filter() {
    let raw = json!({"game_id":520,"width":19,"height":19,"rules":"japanese","komi":0.5,"handicap":2,"initial_player":"white","initial_state":{"black":"djpj","white":""},"moves":[[13,2,-1],[-1,-1,0]],"players":{"black":{"username":"a","rank":16},"white":{"username":"b","rank":32}},"time_control":{"speed":"correspondence"}});
    let g = normalize_ogs(&raw).unwrap();
    assert_eq!(g.setup.len(), 2);
    assert_eq!(g.initial_player, "W");
    assert_eq!(g.speed, "correspondence");
    assert_eq!(g.black_rank, "14k");
    assert_eq!(g.white_rank, "3d");
    assert_eq!(g.moves[1][1], "pass");
}
#[test]
fn sgf_import_handles_setup_escapes_mainline_and_handicap() {
    let g=whole_board::sgf::parse(r"(;SZ[9]KM[0.5]RU[Japanese]HA[2]AB[cc][gg]PL[W]GC[correspondence,ranked]C[escaped\] text];W[cg](;B[gc])(;B[ee]))","fixture.sgf").unwrap();
    assert_eq!(g.moves.len(), 2);
    assert_eq!(g.setup.len(), 2);
    assert_eq!(g.speed, "correspondence");
    assert_eq!(g.handicap, 2);
    assert_eq!(g.moves[1][1], "G7");
    whole_board::go::replay(&g, 2).unwrap();
}
#[test]
fn captures_follow_history_setup_passes_and_the_requested_turn() {
    let g:whole_board::model::Game=serde_json::from_value(serde_json::json!({"id":"captures","size":9,"komi":6.5,"rules":"japanese","initial_player":"B","setup":[["W","B8"],["B","A8"],["B","B9"],["B","C8"]],"moves":[["B","B7"],["W","pass"],["B","pass"]]})).unwrap();
    let (_, c) = whole_board::go::replay_with_captures(&g, 0).unwrap();
    assert_eq!(c.black, 0);
    let (b, c) = whole_board::go::replay_with_captures(&g, 3).unwrap();
    assert_eq!(c.black, 1);
    assert_eq!(c.white, 0);
    assert_eq!(b[1][1], 0);
}
#[test]
fn legal_suicide_is_credited_to_the_opponent_capture_total() {
    let g:whole_board::model::Game=serde_json::from_value(serde_json::json!({"id":"suicide","size":9,"komi":7.5,"rules":"new-zealand","initial_player":"B","setup":[["W","A8"],["W","B9"]],"moves":[["B","A9"]]})).unwrap();
    let (b, c) = whole_board::go::replay_with_captures(&g, 1).unwrap();
    assert_eq!(c.white, 1);
    assert_eq!(c.black, 0);
    assert_eq!(b[0][0], 0);
}
