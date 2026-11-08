use crate::{
    model::{new_id, Attempt, PackJob, Result},
    packs::Service,
};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

pub struct ApiError(pub StatusCode, pub String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn bad(e: String) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, e)
}
fn unavailable(e: String) -> ApiError {
    ApiError(StatusCode::SERVICE_UNAVAILABLE, e)
}
pub fn router(service: Arc<Service>, assets: PathBuf) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/status", get(status))
        .route("/api/progress", get(progress))
        .route("/api/review", get(review_positions))
        .route("/api/new", post(new_problem))
        .route("/api/problems/{id}", get(problem))
        .route("/api/import", post(import_sgf))
        .route("/api/sync", post(sync))
        .route("/api/packs", get(jobs).post(create_pack))
        .route("/api/packs/{id}", get(pack))
        .route("/api/packs/{id}/retry", post(retry_pack))
        .route("/api/packs/{id}/cancel", post(cancel_pack))
        .fallback_service(ServeDir::new(assets).append_index_html_on_directories(true))
        .layer(DefaultBodyLimit::max(5_000_000))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .with_state(service)
}
async fn status(State(s): State<Arc<Service>>) -> std::result::Result<Json<Value>, ApiError> {
    let cohorts = s.db.cohorts().map_err(unavailable)?;
    let mut sizes = json!({"9":0,"13":0,"19":0});
    let mut games = 0;
    for c in &cohorts {
        games += c.count;
        if let Some(v) = sizes.get_mut(c.size.to_string()) {
            *v = json!(v.as_u64().unwrap_or(0) + c.count as u64);
        }
    }
    Ok(Json(
        json!({"api_version":2,"backend":"rust","version":env!("CARGO_PKG_VERSION"),"games":games,"sizes":sizes,"cohorts":cohorts,"raw_records":s.db.raw_count().map_err(unavailable)?,"engine":s.engine.config.model.is_file(),"model":s.engine.identity,"progress":s.db.progress().map_err(unavailable)?,"jobs":s.db.jobs().map_err(unavailable)?}),
    ))
}
async fn progress(State(s): State<Arc<Service>>) -> std::result::Result<Json<Value>, ApiError> {
    Ok(Json(s.db.progress().map_err(unavailable)?))
}
async fn review_positions(
    State(s): State<Arc<Service>>,
) -> std::result::Result<Json<Value>, ApiError> {
    Ok(Json(s.db.review_positions().map_err(unavailable)?))
}
#[derive(Deserialize)]
struct NewRequest {
    #[serde(default = "mixed")]
    size: String,
    #[serde(default = "live")]
    speed: String,
}
fn mixed() -> String {
    "mixed".into()
}
fn live() -> String {
    "live".into()
}
async fn new_problem(
    State(s): State<Arc<Service>>,
    Json(b): Json<NewRequest>,
) -> std::result::Result<Json<Value>, ApiError> {
    if !["mixed", "9", "13", "19"].contains(&b.size.as_str())
        || !["live", "all", "realtime", "correspondence"].contains(&b.speed.as_str())
    {
        return Err(bad("Invalid practice settings".into()));
    }
    Ok(Json(
        serde_json::to_value(
            s.next_problem(&b.size, &b.speed)
                .await
                .map_err(unavailable)?
                .problem,
        )
        .map_err(|e| unavailable(e.to_string()))?,
    ))
}
async fn problem(
    State(s): State<Arc<Service>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<Value>, ApiError> {
    let p =
        s.db.problem(&id)
            .map_err(unavailable)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Problem not found".into()))?;
    Ok(Json(
        serde_json::to_value(p.problem).map_err(|e| unavailable(e.to_string()))?,
    ))
}
#[derive(Deserialize)]
struct SyncRequest {
    attempts: Vec<Attempt>,
}
async fn sync(
    State(s): State<Arc<Service>>,
    Json(b): Json<SyncRequest>,
) -> std::result::Result<Json<Value>, ApiError> {
    if b.attempts.len() > 100 {
        return Err(bad("Sync at most 100 attempts per request".into()));
    }
    let mut accepted = vec![];
    let mut rejected = vec![];
    for attempt in b.attempts {
        let id = attempt.id.clone();
        match s.sync_attempt(attempt).await {
            Ok(v) => accepted.push(v),
            Err(e) => rejected.push(json!({"id":id,"error":e})),
        }
    }
    Ok(Json(
        json!({"accepted":accepted,"rejected":rejected,"progress":s.db.progress().map_err(unavailable)?}),
    ))
}
#[derive(Deserialize)]
struct PackRequest {
    #[serde(default = "new_id")]
    id: String,
    #[serde(default = "count")]
    count: usize,
    #[serde(default = "mixed")]
    size: String,
    #[serde(default = "live")]
    speed: String,
}
fn count() -> usize {
    25
}
async fn create_pack(
    State(s): State<Arc<Service>>,
    Json(b): Json<PackRequest>,
) -> std::result::Result<Json<PackJob>, ApiError> {
    Ok(Json(
        s.create_job(b.id, b.count, b.size, b.speed).map_err(bad)?,
    ))
}
async fn jobs(State(s): State<Arc<Service>>) -> std::result::Result<Json<Value>, ApiError> {
    Ok(Json(json!({"jobs":s.db.jobs().map_err(unavailable)?})))
}
async fn pack(
    State(s): State<Arc<Service>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<Value>, ApiError> {
    let job =
        s.db.job(&id)
            .map_err(unavailable)?
            .ok_or(ApiError(StatusCode::NOT_FOUND, "Pack not found".into()))?;
    let ready =
        s.db.pack_problems(&id)
            .map_err(unavailable)?
            .into_iter()
            .filter(|p| {
                p.problem.analysis.complete
                    && p.problem
                        .legal_moves
                        .iter()
                        .all(|v| p.problem.analysis.candidates.contains_key(v))
            })
            .map(|p| p.problem)
            .collect::<Vec<_>>();
    Ok(Json(json!({"job":job,"problems":ready})))
}
async fn change_job(s: &Service, id: &str, state: &str) -> Result<PackJob> {
    let mut j = s.db.job(id)?.ok_or("Pack not found")?;
    if state == "queued" && !["failed", "cancelled"].contains(&j.state.as_str()) {
        return Err("Only failed or cancelled packs can be retried".into());
    }
    j.state = state.into();
    j.error = None;
    s.db.save_job(&j)?;
    Ok(j)
}
async fn retry_pack(
    State(s): State<Arc<Service>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<PackJob>, ApiError> {
    Ok(Json(change_job(&s, &id, "queued").await.map_err(bad)?))
}
async fn cancel_pack(
    State(s): State<Arc<Service>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<PackJob>, ApiError> {
    Ok(Json(change_job(&s, &id, "cancelled").await.map_err(bad)?))
}

#[derive(Deserialize)]
struct ImportRequest {
    sgf: String,
    #[serde(default)]
    filename: String,
}
async fn import_sgf(
    State(s): State<Arc<Service>>,
    Json(b): Json<ImportRequest>,
) -> std::result::Result<Json<Value>, ApiError> {
    use sha2::{Digest, Sha256};
    if b.sgf.len() > 5_000_000 {
        return Err(bad("SGF must be under 5 MB".into()));
    }
    let dir = s.engine.config.data_dir.join("uploads");
    std::fs::create_dir_all(&dir).map_err(|e| unavailable(e.to_string()))?;
    let digest = format!("{:x}", Sha256::digest(b.sgf.as_bytes()));
    std::fs::write(dir.join(format!("{digest}.sgf")), &b.sgf)
        .map_err(|e| unavailable(e.to_string()))?;
    let name = b.filename.chars().take(200).collect::<String>();
    let g = crate::sgf::parse(&b.sgf, &name)
        .map_err(|e| bad(format!("Original upload retained. {e}")))?;
    let inserted = s.db.add_game(&g).map_err(unavailable)?;
    Ok(Json(json!({"id":g.id,"inserted":inserted})))
}
