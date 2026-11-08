use crate::{
    db::Db,
    model::{new_id, Game, Result},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};

#[derive(Clone)]
pub struct EngineConfig {
    pub binary: PathBuf,
    pub model: PathBuf,
    pub data_dir: PathBuf,
    pub visits: u64,
    pub timeout_secs: u64,
}
struct Process {
    _child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
}
pub struct Engine {
    pub config: EngineConfig,
    pub identity: String,
    db: Db,
    process: Mutex<Option<Process>>,
}
impl Engine {
    pub fn new(config: EngineConfig, db: Db) -> Self {
        let meta = std::fs::metadata(&config.model).ok();
        let identity = format!(
            "{}:{}:{:?}:{}",
            config
                .model
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            meta.as_ref().map(|m| m.len()).unwrap_or(0),
            meta.and_then(|m| m.modified().ok()),
            config.visits
        );
        Self {
            config,
            identity,
            db,
            process: Mutex::new(None),
        }
    }
    fn start(&self) -> Result<Process> {
        if !self.config.model.is_file() {
            return Err("KataGo model missing; configure WB_KATAGO_MODEL".into());
        }
        let dir = self.config.data_dir.join("engine");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let config = dir.join("analysis.cfg");
        std::fs::write(&config,format!("logDir = {}\nreportAnalysisWinratesAs = BLACK\nnumAnalysisThreads = 1\nnumSearchThreadsPerAnalysisThread = 4\nnnMaxBatchSize = 8\nnnCacheSizePowerOfTwo = 18\nnnMutexPoolSizePowerOfTwo = 12\nhomeDataDir = {}\n",dir.join("logs").display(),dir.join("home").display())).map_err(|e|e.to_string())?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("stderr.log"))
            .map_err(|e| e.to_string())?;
        let mut child = Command::new(&self.config.binary)
            .args(["analysis", "-config"])
            .arg(config)
            .arg("-model")
            .arg(&self.config.model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Starting KataGo: {e}"))?;
        let input = child.stdin.take().ok_or("Missing KataGo stdin")?;
        let output = BufReader::new(child.stdout.take().ok_or("Missing KataGo stdout")?).lines();
        Ok(Process {
            _child: child,
            input,
            output,
        })
    }
    pub async fn analyze(&self, g: &Game, turn: usize, forced: Option<&str>) -> Result<Value> {
        if turn > g.moves.len() {
            return Err("Invalid analysis turn".into());
        }
        let mut request = json!({"moves":g.moves[..turn],"initialStones":g.setup,"initialPlayer":g.initial_player,"rules":g.rules,"komi":g.komi,"boardXSize":g.size,"boardYSize":g.size,"analyzeTurns":[turn],"maxVisits":self.config.visits,"includeOwnership":true,"includePolicy":true,"analysisPVLen":8});
        if let Some(v) = forced {
            request["allowMoves"] =
                json!([{"player":crate::go::to_play(g,turn),"moves":[v],"untilDepth":1}]);
        }
        let key = format!(
            "wb-v1-{:x}",
            Sha256::digest(format!("{}:{request}", self.identity))
        );
        if let Some(v) = self.db.cached(&key)? {
            return Ok(v);
        }
        let mut process = self.process.lock().await;
        if let Some(v) = self.db.cached(&key)? {
            return Ok(v);
        }
        if process.is_none() {
            *process = Some(self.start()?);
        }
        let id = new_id();
        request["id"] = json!(id);
        let p = process.as_mut().ok_or("Engine unavailable")?;
        let exchange = async {
            p.input
                .write_all(format!("{request}\n").as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            p.input.flush().await.map_err(|e| e.to_string())?;
            loop {
                let line = p
                    .output
                    .next_line()
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("KataGo exited; check the engine stderr log")?;
                let v: Value = serde_json::from_str(&line)
                    .map_err(|e| format!("Invalid KataGo response: {e}"))?;
                if v["id"].as_str() != Some(&id) {
                    continue;
                }
                if let Some(error) = v.get("error") {
                    return Err(format!("KataGo: {error}"));
                }
                if v["isDuringSearch"].as_bool() == Some(true) || v.get("rootInfo").is_none() {
                    continue;
                }
                if v["rootInfo"]["scoreLead"]
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .is_none()
                {
                    return Err("Engine did not return a finite score".into());
                }
                if v["ownership"].as_array().map(Vec::len) != Some(g.size * g.size)
                    || v["policy"].as_array().map(Vec::len) != Some(g.size * g.size + 1)
                {
                    return Err("Incomplete engine ownership/legal-move data".into());
                }
                if let Some(m) = forced {
                    if !v["moveInfos"].as_array().is_some_and(|ms| {
                        ms.iter().any(|x| {
                            x["move"].as_str() == Some(m) && x["visits"].as_u64().unwrap_or(0) > 0
                        })
                    }) {
                        return Err("Engine did not evaluate the requested candidate".into());
                    }
                }
                return Ok(v);
            }
        };
        let result =
            match tokio::time::timeout(Duration::from_secs(self.config.timeout_secs), exchange)
                .await
            {
                Ok(v) => v,
                Err(_) => {
                    Err("KataGo request timed out; retry when the engine is available".into())
                }
            };
        match result {
            Ok(v) => {
                self.db.cache(&key, &v)?;
                Ok(v)
            }
            Err(e) => {
                *process = None;
                Err(e)
            }
        }
    }
}
