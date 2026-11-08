use serde_json::json;
use whole_board::{db::Db, model::Game};
fn temporary() -> (std::path::PathBuf, Db) {
    let p = std::env::temp_dir().join(format!("whole-board-test-{}", whole_board::model::new_id()));
    std::fs::create_dir_all(&p).unwrap();
    let db = Db::open(p.join("test.sqlite")).unwrap();
    (p, db)
}
fn game(id: &str, size: usize, rank: &str) -> Game {
    serde_json::from_value(json!({"id":id,"size":size,"komi":6.5,"rules":"japanese","moves":[["B","C3"]],"black_rank":rank,"white_rank":rank,"speed":"live","source_url":format!("https://online-go.com/game/{id}")})).unwrap()
}
#[test]
fn cohort_selection_includes_all_ranks_and_never_loads_whole_corpus() {
    let (p, db) = temporary();
    db.add_game(&game("1", 9, "20k")).unwrap();
    db.add_game(&game("2", 9, "3d")).unwrap();
    db.add_game(&game("3", 19, "5k")).unwrap();
    assert_eq!(db.game_at(9, "live", 0).unwrap().unwrap().black_rank, "20k");
    assert_eq!(db.game_at(9, "live", 1).unwrap().unwrap().black_rank, "3d");
    assert_eq!(
        db.cohorts().unwrap().iter().map(|c| c.count).sum::<usize>(),
        3
    );
    assert!(!db.add_game(&game("1", 9, "20k")).unwrap());
    drop(db);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn raw_records_survive_unsupported_boards_and_import_checkpoint_is_atomic() {
    let (p, db) = temporary();
    db.import_batch(
        "source",
        &[(
            "a".into(),
            "{\"width\":25}".into(),
            None,
            "unsupported".into(),
        )],
        1,
    )
    .unwrap();
    assert_eq!(db.import_cursor("source").unwrap(), 1);
    assert_eq!(db.raw_count().unwrap(), 1);
    db.import_batch(
        "source",
        &[(
            "a".into(),
            "{\"width\":25}".into(),
            None,
            "unsupported".into(),
        )],
        1,
    )
    .unwrap();
    assert_eq!(db.raw_count().unwrap(), 1);
    drop(db);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn streaming_import_resumes_without_losing_raw_unsupported_records() {
    let (p, db) = temporary();
    let archive = p.join("records.jsonl");
    let raw = json!({"game_id":123,"width":9,"height":9,"komi":6.5,"rules":"japanese","moves":[[2,2,0]],"players":{},"time_control":{"speed":"live"}});
    std::fs::write(
        &archive,
        format!("{}\n{}\n", raw, json!({"game_id":124,"width":25})),
    )
    .unwrap();
    let one = whole_board::archive::import_json(&db, &archive, 1).unwrap();
    assert_eq!(one.read, 1);
    assert_eq!(one.normalized, 1);
    let two = whole_board::archive::import_json(&db, &archive, 0).unwrap();
    assert_eq!(two.previous_line, 1);
    assert_eq!(two.retained_with_issue, 1);
    assert_eq!(db.raw_count().unwrap(), 2);
    assert_eq!(db.cohorts().unwrap()[0].count, 1);
    drop(db);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn distinct_snapshots_with_the_same_game_id_are_both_retained() {
    let (p, db) = temporary();
    let file = p.join("snapshots.jsonl");
    std::fs::write(&file,"{\"game_id\":1,\"width\":25,\"snapshot\":\"first\"}\n{\"game_id\":1,\"width\":25,\"snapshot\":\"later\"}\n").unwrap();
    let report = whole_board::archive::import_json(&db, &file, 0).unwrap();
    assert_eq!(report.read, 2);
    assert_eq!(db.raw_count().unwrap(), 2);
    assert_eq!(db.import_cursor(&report.source).unwrap(), 2);
    drop(db);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn prototype_progress_is_migrated_once_without_changing_its_rank_breakdown() {
    let (p, db) = temporary();
    db.connect().unwrap().execute_batch("CREATE TABLE attempts(session TEXT,stage TEXT,size INTEGER,rank TEXT,correct INTEGER,detail TEXT,created TEXT); INSERT INTO attempts VALUES ('old','winner',19,'9k/3d',1,'{\"answer\":\"W\"}','2026-11-07 12:00:00');").unwrap();
    let migrated = Db::open(&db.path).unwrap();
    assert_eq!(migrated.progress().unwrap()["total"], 1);
    assert_eq!(migrated.progress().unwrap()["breakdown"][0]["rank"], "3k");
    let reopened = Db::open(&db.path).unwrap();
    assert_eq!(reopened.progress().unwrap()["total"], 1);
    drop(reopened);
    drop(migrated);
    drop(db);
    std::fs::remove_dir_all(p).unwrap();
}
