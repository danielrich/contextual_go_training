use axum::{body::Body, http::Request};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;
use whole_board::{
    db::Db,
    engine::{Engine, EngineConfig},
    grade, http,
    model::*,
    packs::Service,
};
fn setup() -> (PathBuf, Arc<Service>) {
    let dir = std::env::temp_dir().join(format!("whole-board-api-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Db::open(dir.join("test.sqlite")).unwrap();
    let engine = Engine::new(
        EngineConfig {
            binary: "/missing".into(),
            model: "/missing".into(),
            data_dir: dir.clone(),
            visits: 200,
            timeout_secs: 1,
        },
        db.clone(),
    );
    (dir, Arc::new(Service::new(db, engine)))
}
fn saved() -> StoredProblem {
    serde_json::from_value(json!({"game":{"id":"g","size":9,"komi":6.5,"rules":"japanese","moves":[["B","C7"]]},"problem":{"id":"p1","size":9,"board":vec![vec![0;9];9],"turn":1,"to_play":"W","komi":6.5,"rules":"japanese","source":"test","source_url":"","source_game_id":"g","black":"a","white":"b","black_rank":"20k","white_rank":"3d","rank_provenance":"fixture","date":"","speed":"live","handicap":0,"sample":false,"last_move":null,"groups":[],"legal_moves":["D4","E5","pass"],"analysis":{"score":-4.,"ownership":vec![0.;81],"model":"test","visits":200,"grader_version":grade::VERSION,"candidates":{"D4":{"move":"D4","score":-4.,"visits":200,"pv":[]},"E5":{"move":"E5","score":-3.,"visits":200,"pv":[]},"pass":{"move":"pass","score":-4.,"visits":200,"pv":[]}},"suggestions":[],"complete":true}}})).unwrap()
}
async fn post(service: Arc<Service>, value: Value) -> Value {
    let router = http::router(
        service,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
    );
    let response = router
        .oneshot(
            Request::post("/api/sync")
                .header("Content-Type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 2_000_000)
            .await
            .unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn server_regrades_answers_and_repeated_sync_is_idempotent() {
    let (dir, s) = setup();
    s.db.save_problem(&saved()).unwrap();
    let attempt = json!({"id":"a1","problem_id":"p1","stage":"winner","answer":"B","correct":true,"reasoning":"My actual reasoning","created":now(),"grader_version":grade::VERSION});
    let body = json!({"attempts":[attempt.clone()]});
    let one = post(s.clone(), body.clone()).await;
    assert_eq!(one["accepted"][0]["feedback"]["correct"], false);
    assert_eq!(one["progress"]["total"], 1);
    let two = post(s.clone(), body).await;
    assert_eq!(two["progress"]["total"], 1);
    assert_eq!(two["progress"]["breakdown"][0]["rank"], "9k");
    let mut changed = attempt;
    changed["answer"] = json!("W");
    let three = post(s.clone(), json!({"attempts":[changed]})).await;
    assert_eq!(three["rejected"].as_array().unwrap().len(), 1);
    assert_eq!(three["progress"]["total"], 1);
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn incomplete_problems_are_not_advertised_as_offline_ready_and_survive_restart() {
    let (dir, s) = setup();
    let mut p = saved();
    p.problem.analysis.complete = false;
    p.problem.analysis.candidates.remove("pass");
    s.db.save_problem(&p).unwrap();
    let job = PackJob {
        id: "j1".into(),
        requested: 1,
        size: "9".into(),
        speed: "live".into(),
        state: "running".into(),
        completed: 0,
        error: None,
        current_moves_done: 2,
        current_moves_total: 3,
    };
    s.db.save_job(&job).unwrap();
    s.db.link_problem("j1", 0, "p1").unwrap();
    let router = http::router(
        s.clone(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
    );
    let response = router
        .oneshot(Request::get("/api/packs/j1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 2_000_000)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["problems"].as_array().unwrap().len(), 0);
    let reopened = Db::open(&s.db.path).unwrap();
    assert_eq!(reopened.unfinished_jobs().unwrap()[0].current_moves_done, 2);
    assert!(reopened.pack_problems("j1").unwrap()[0]
        .problem
        .analysis
        .candidates
        .contains_key("D4"));
    drop(reopened);
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn cancellation_cannot_be_overwritten_by_late_worker_progress() {
    let (dir, s) = setup();
    let mut job = PackJob {
        id: "j1".into(),
        requested: 1,
        size: "9".into(),
        speed: "live".into(),
        state: "running".into(),
        completed: 0,
        error: None,
        current_moves_done: 0,
        current_moves_total: 3,
    };
    s.db.save_job(&job).unwrap();
    let stale = job.clone();
    job.state = "cancelled".into();
    s.db.save_job(&job).unwrap();
    assert!(!s.db.save_job_progress(&stale).unwrap());
    assert_eq!(s.db.job("j1").unwrap().unwrap().state, "cancelled");
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn progress_measures_each_evaluated_candidate_loss_by_source_ranks() {
    let dir = std::env::temp_dir().join(format!("wb-quality-{}", whole_board::model::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = whole_board::db::Db::open(dir.join("test.sqlite")).unwrap();
    let p = saved().problem;
    let a = whole_board::model::Attempt {
        id: "quality-one".into(),
        problem_id: p.id.clone(),
        stage: "candidates".into(),
        answer: serde_json::json!(["A1", "B1", "pass"]),
        reasoning: String::new(),
        created: 1700000000,
        grader_version: whole_board::grade::VERSION.into(),
        review: false,
        session_id: String::new(),
    };
    let f = serde_json::json!({"correct":false,"pending":false,"candidates":[{"loss":0.0,"correct":true},{"loss":1.5,"correct":true},{"loss":7.5,"correct":false}]});
    db.record_attempt(&a, &p, &f).unwrap();
    db.record_attempt(&a, &p, &f).unwrap();
    let pending = whole_board::model::Attempt {
        id: "quality-pending".into(),
        ..a
    };
    db.record_attempt(&pending,&p,&serde_json::json!({"correct":null,"pending":true,"candidates":[{"loss":90.0,"correct":false}]})).unwrap();
    let q = db.progress().unwrap()["candidate_quality"].clone();
    assert_eq!(q["total_moves"], 3);
    assert_eq!(q["sound_moves"], 2);
    assert_eq!(q["average_loss"], 3.0);
    assert_eq!(q["breakdown"][0]["rank"], "9k");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn repeated_positions_only_contribute_to_review_accuracy() {
    let (dir, s) = setup();
    let p = saved();
    s.db.save_problem(&p).unwrap();
    let a:Attempt=serde_json::from_value(json!({"id":"first-view","problem_id":"p1","stage":"winner","answer":"W","created":1700000000,"grader_version":grade::VERSION})).unwrap();
    s.db.record_attempt(&a, &p.problem, &json!({"correct":false}))
        .unwrap();
    let b:Attempt=serde_json::from_value(json!({"id":"second-view","problem_id":"p1","stage":"winner","answer":"W","created":1700000001,"grader_version":grade::VERSION})).unwrap();
    s.db.record_attempt(&b, &p.problem, &json!({"correct":true}))
        .unwrap();
    let progress = s.db.progress().unwrap();
    assert_eq!(progress["total"], 1);
    assert_eq!(progress["review"]["total"], 1);
    assert_eq!(progress["skills"]["winner"]["correct"], 0);
    assert_eq!(
        s.db.review_positions().unwrap().as_array().unwrap().len(),
        1
    );
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn prepared_positions_are_claimed_once_and_match_engine_identity() {
    let (dir, s) = setup();
    let p = saved();
    s.db.save_problem(&p).unwrap();
    s.db.queue_ready(&p).unwrap();
    assert_eq!(s.db.ready_count(9, "live", "test").unwrap(), 1);
    assert!(s.db.take_ready(9, "live", "other").unwrap().is_none());
    assert!(s.db.take_ready(9, "all", "test").unwrap().is_some());
    assert!(s.db.take_ready(9, "live", "test").unwrap().is_none());
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn first_session_questions_stay_main_but_a_later_session_is_entirely_review() {
    let (dir, s) = setup();
    let p = saved();
    s.db.save_problem(&p).unwrap();
    for (id, session, stage) in [
        ("one", "original", "winner"),
        ("two", "original", "score"),
        ("three", "revisit", "candidates"),
    ] {
        let a:Attempt=serde_json::from_value(json!({"id":id,"session_id":session,"problem_id":"p1","stage":stage,"answer":"W","created":1700000000,"grader_version":grade::VERSION})).unwrap();
        s.db.record_attempt(&a, &p.problem, &json!({"correct":true}))
            .unwrap();
    }
    let p = s.db.progress().unwrap();
    assert_eq!(p["total"], 2);
    assert_eq!(p["review"]["total"], 1);
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
