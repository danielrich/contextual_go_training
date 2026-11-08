use std::{collections::HashMap, path::PathBuf, sync::Arc};
use whole_board::{
    archive,
    db::Db,
    engine::{Engine, EngineConfig},
    http,
    model::Result,
    packs::Service,
};

fn option(args: &HashMap<String, String>, key: &str, env: &str, default: &str) -> String {
    args.get(key)
        .cloned()
        .or_else(|| std::env::var(env).ok())
        .unwrap_or_else(|| default.into())
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("whole-board: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|v| v == "--help" || v == "-h") {
        println!("Whole Board\n\nwhole-board serve [--host 127.0.0.1] [--port 8765] [--db data/trainer.sqlite] [--static-dir static] [--cert cert.pem --key key.pem]\nwhole-board import-json --input archive.json.gz [--limit 0] [--db data/trainer.sqlite]\nwhole-board prepare-pack [--count 25] [--size mixed] [--speed live] [--db data/trainer.sqlite]\n\nEngine: WB_KATAGO_BIN, WB_KATAGO_MODEL, WB_KATAGO_VISITS (default 200), WB_ENGINE_TIMEOUT (default 120).\nService: WB_DATA_DIR, WB_DB, WB_HOST, WB_PORT, WB_STATIC_DIR, WB_TLS_CERT, WB_TLS_KEY.");
        return Ok(());
    }
    let command = raw
        .first()
        .filter(|v| !v.starts_with('-'))
        .map(String::as_str)
        .unwrap_or("serve");
    let offset = usize::from(raw.first().is_some_and(|s| !s.starts_with('-')));
    let mut args = HashMap::new();
    let mut i = offset;
    while i < raw.len() {
        let key = raw[i].strip_prefix("--").ok_or("Expected a --flag")?;
        if ![
            "data-dir",
            "db",
            "input",
            "limit",
            "katago",
            "model",
            "visits",
            "engine-timeout",
            "host",
            "port",
            "static-dir",
            "cert",
            "key",
            "count",
            "size",
            "speed",
        ]
        .contains(&key)
        {
            return Err(format!("Unknown flag --{key}"));
        }
        let value = raw.get(i + 1).ok_or("Each flag requires a value")?;
        if value.starts_with("--") {
            return Err("Each flag requires a value".into());
        }
        args.insert(key.to_string(), value.clone());
        i += 2;
    }
    let data = PathBuf::from(option(&args, "data-dir", "WB_DATA_DIR", "data"));
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let default_db = data.join("trainer.sqlite");
    let db = Db::open(option(&args, "db", "WB_DB", &default_db.to_string_lossy()))?;
    if command == "import-json" {
        let path = args.get("input").ok_or("Provide --input archive.json.gz")?;
        let limit = args
            .get("limit")
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|_| "Invalid limit")?
            .unwrap_or(0);
        let report = archive::import_json(&db, &PathBuf::from(path), limit)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let config = EngineConfig {
        binary: PathBuf::from(option(&args, "katago", "WB_KATAGO_BIN", "katago")),
        model: PathBuf::from(option(
            &args,
            "model",
            "WB_KATAGO_MODEL",
            "/opt/homebrew/share/katago/g170e-b20c256x2-s5303129600-d1228401921.bin.gz",
        )),
        data_dir: data,
        visits: option(&args, "visits", "WB_KATAGO_VISITS", "200")
            .parse::<u64>()
            .map_err(|_| "Invalid visits")?,
        timeout_secs: option(&args, "engine-timeout", "WB_ENGINE_TIMEOUT", "120")
            .parse::<u64>()
            .map_err(|_| "Invalid timeout")?,
    };
    if config.visits == 0 || config.timeout_secs == 0 {
        return Err("Visits and timeout must be positive".into());
    }
    let engine = Engine::new(config, db.clone());
    let service = Arc::new(Service::new(db, engine));
    if command == "prepare-pack" {
        let count = args
            .get("count")
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|_| "Invalid count")?
            .unwrap_or(25);
        let job = service.create_job(
            whole_board::model::new_id(),
            count,
            args.get("size").cloned().unwrap_or("mixed".into()),
            args.get("speed").cloned().unwrap_or("live".into()),
        )?;
        let id = job.id.clone();
        service.run_job(job).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&service.db.job(&id)?).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if command != "serve" {
        return Err("Unknown command; use --help".into());
    }
    let cert = option(&args, "cert", "WB_TLS_CERT", "");
    let key = option(&args, "key", "WB_TLS_KEY", "");
    if cert.is_empty() != key.is_empty() {
        return Err("Provide both a TLS certificate and key".into());
    }
    let host = option(&args, "host", "WB_HOST", "127.0.0.1");
    let port = option(
        &args,
        "port",
        "WB_PORT",
        if cert.is_empty() { "8765" } else { "8443" },
    );
    let address = format!("{host}:{port}")
        .parse::<std::net::SocketAddr>()
        .map_err(|e| e.to_string())?;
    let assets = PathBuf::from(option(&args, "static-dir", "WB_STATIC_DIR", "static"));
    if !assets.join("index.html").is_file() {
        return Err("Static directory must contain index.html".into());
    }
    let app = http::router(service.clone(), assets);
    let prefetch = service.clone();
    tokio::spawn(async move {
        prefetch.prefetch().await;
    });
    let worker = service.clone();
    tokio::spawn(async move {
        worker.worker().await;
    });
    println!(
        "Whole Board {} at {}://{address}",
        env!("CARGO_PKG_VERSION"),
        if cert.is_empty() { "http" } else { "https" }
    );
    if cert.is_empty() {
        let listener = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|e| e.to_string())?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown())
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key)
            .await
            .map_err(|e| e.to_string())?;
        let handle = axum_server::Handle::new();
        let stopper = handle.clone();
        tokio::spawn(async move {
            shutdown().await;
            stopper.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
        });
        axum_server::bind_rustls(address, tls)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
