use crate::{db::Db, engine::Engine, go, grade, model::*};
use rand::Rng;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Service {
    pub db: Db,
    pub engine: Engine,
    pub problem_lock: Mutex<()>,
}
impl Service {
    pub fn new(db: Db, engine: Engine) -> Self {
        Self {
            db,
            engine,
            problem_lock: Mutex::new(()),
        }
    }
    pub fn choose_game(&self, size: &str, speed: &str) -> Result<Game> {
        let cohorts = self.db.cohorts()?;
        let available: Vec<_> = cohorts
            .into_iter()
            .filter(|c| {
                [9, 13, 19].contains(&c.size)
                    && (size == "mixed" || size.parse::<usize>().ok() == Some(c.size))
                    && (speed == "all"
                        || (speed == "realtime"
                            && ["live", "blitz", "rapid"].contains(&c.speed.as_str()))
                        || c.speed == speed)
                    && c.count > 0
            })
            .collect();
        if available.is_empty() {
            return Err("No games match this board size and speed. Import an archive or choose another practice setting.".into());
        }
        let mut rng = rand::thread_rng();
        let selected_size = if size == "mixed" {
            let mut options = vec![];
            for n in [9, 13, 19] {
                if available.iter().any(|c| c.size == n) {
                    for _ in 0..if n == 19 { 6 } else { 2 } {
                        options.push(n);
                    }
                }
            }
            options[rng.gen_range(0..options.len())]
        } else {
            size.parse().map_err(|_| "Invalid board size")?
        };
        let options: Vec<_> = available
            .iter()
            .filter(|c| c.size == selected_size)
            .collect();
        let total: usize = options.iter().map(|c| c.count).sum();
        let mut offset = rng.gen_range(0..total);
        for c in options {
            if offset < c.count {
                return self
                    .db
                    .game_at(c.size, &c.speed, offset)?
                    .ok_or("Corpus slot missing".into());
            }
            offset -= c.count;
        }
        Err("Corpus selection failed".into())
    }
    pub async fn next_problem(&self, size: &str, speed: &str) -> Result<StoredProblem> {
        let selected = if size == "mixed" {
            self.choose_game(size, speed)?.size
        } else {
            size.parse().map_err(|_| "Invalid board size")?
        };
        if let Some(p) = self.db.take_ready(selected, speed, &self.engine.identity)? {
            return Ok(p);
        }
        self.new_problem(&selected.to_string(), speed).await
    }
    pub async fn prefetch(self: Arc<Self>) {
        loop {
            for size in [9, 19, 13] {
                if !self.engine.config.model.is_file() {
                    break;
                }
                if self
                    .db
                    .ready_count(size, "live", &self.engine.identity)
                    .unwrap_or(2)
                    >= 2
                {
                    continue;
                }
                if let Ok(p) = self.new_problem(&size.to_string(), "live").await {
                    let _ = self.db.queue_ready(&p);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    pub async fn new_problem(&self, size: &str, speed: &str) -> Result<StoredProblem> {
        let mut last_error = String::new();
        for _ in 0..12 {
            let g = self.choose_game(size, speed)?;
            let upper = go::last_playable_turn(&g);
            if upper == 0 {
                last_error = "Selected record has no played positions".into();
                continue;
            }
            let turn = rand::thread_rng().gen_range(1..=upper);
            match self.prepare(g, turn).await {
                Ok(p) => return Ok(p),
                Err(e) => {
                    last_error = e;
                    if !self.engine.config.model.is_file() {
                        break;
                    }
                }
            }
        }
        Err(format!("Could not prepare a position: {last_error}"))
    }
    pub async fn prepare(&self, g: Game, turn: usize) -> Result<StoredProblem> {
        let board = go::replay(&g, turn)?;
        let root = self.engine.analyze(&g, turn, None).await?;
        let legal_moves = root["policy"]
            .as_array()
            .ok_or("Missing engine legal mask")?
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                v.as_f64().filter(|v| *v >= 0.).map(|_| {
                    if i == g.size * g.size {
                        "pass".into()
                    } else {
                        go::vertex(i % g.size, i / g.size, g.size)
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut ranked = root["moveInfos"]
            .as_array()
            .ok_or("Missing engine move suggestions")?
            .iter()
            .collect::<Vec<_>>();
        ranked.sort_by_key(|m| m["order"].as_u64().unwrap_or(u64::MAX));
        let suggestions = ranked
            .into_iter()
            .take(3)
            .map(move_eval)
            .collect::<Result<Vec<_>>>()?;
        let problem = Problem {
            id: new_id(),
            size: g.size,
            groups: go::groups(&board),
            board,
            turn,
            to_play: go::to_play(&g, turn),
            komi: g.komi,
            rules: g.rules.clone(),
            source: g.source.clone(),
            source_url: g.source_url.clone(),
            source_game_id: g.id.clone(),
            black: g.black.clone(),
            white: g.white.clone(),
            black_rank: g.black_rank.clone(),
            white_rank: g.white_rank.clone(),
            rank_provenance: g.rank_provenance.clone(),
            date: g.date.clone(),
            speed: g.speed.clone(),
            handicap: g.handicap,
            sample: g.sample,
            last_move: turn.checked_sub(1).map(|i| g.moves[i][1].clone()),
            legal_moves,
            analysis: Analysis {
                score: root["rootInfo"]["scoreLead"]
                    .as_f64()
                    .ok_or("Missing engine score")?,
                ownership: serde_json::from_value(root["ownership"].clone())
                    .map_err(|e| e.to_string())?,
                model: self.engine.identity.clone(),
                visits: self.engine.config.visits,
                grader_version: grade::VERSION.into(),
                candidates: Default::default(),
                suggestions,
                complete: false,
            },
        };
        let stored = StoredProblem { problem, game: g };
        self.db.save_problem(&stored)?;
        Ok(stored)
    }
    pub async fn evaluate_move(&self, stored: &mut StoredProblem, m: &str) -> Result<()> {
        if stored.problem.analysis.model != self.engine.identity {
            return Err("This problem uses a different engine model; keep its existing feedback and prepare a new problem for additional analysis".into());
        }
        if !stored.problem.legal_moves.iter().any(|v| v == m) {
            return Err("Illegal candidate".into());
        }
        if stored.problem.analysis.candidates.contains_key(m) {
            return Ok(());
        }
        let value = self
            .engine
            .analyze(&stored.game, stored.problem.turn, Some(m))
            .await?;
        let info = value["moveInfos"]
            .as_array()
            .and_then(|ms| ms.iter().find(|v| v["move"].as_str() == Some(m)))
            .ok_or("Candidate was not evaluated")?;
        stored
            .problem
            .analysis
            .candidates
            .insert(m.to_string(), move_eval(info)?);
        stored.problem.analysis.complete = stored
            .problem
            .legal_moves
            .iter()
            .all(|v| stored.problem.analysis.candidates.contains_key(v));
        self.db.save_problem(stored)?;
        Ok(())
    }
    pub async fn sync_attempt(&self, a: Attempt) -> Result<Value> {
        if !valid_id(&a.id)
            || !valid_id(&a.problem_id)
            || a.reasoning.len() > 20000
            || a.created == 0
            || a.created > now() + 86400
        {
            return Err("Invalid attempt metadata".into());
        }
        if let Some((old, feedback)) = self.db.existing_attempt(&a.id)? {
            if serde_json::to_value(&old).map_err(|e| e.to_string())?
                != serde_json::to_value(&a).map_err(|e| e.to_string())?
            {
                return Err("Attempt ID already belongs to different content".into());
            }
            return Ok(json!({"id":a.id,"feedback":feedback}));
        }
        let _lock = self.problem_lock.lock().await;
        let mut p=self.db.problem(&a.problem_id)?.ok_or("Problem not found on this server; retain the offline attempt and restore the server database")?;
        if ![grade::VERSION, "whole-board-grader-v1"].contains(&a.grader_version.as_str()) {
            return Err("Grader version mismatch; offline answer retained".into());
        }
        let initial =
            grade::grade_with_version(&p.problem, &a.stage, &a.answer, &a.grader_version)?;
        if initial["pending"] == true {
            for m in initial["missing"]
                .as_array()
                .ok_or("Missing pending candidates")?
            {
                self.evaluate_move(&mut p, m.as_str().ok_or("Invalid candidate")?)
                    .await?;
            }
        }
        let feedback =
            grade::grade_with_version(&p.problem, &a.stage, &a.answer, &a.grader_version)?;
        if feedback["pending"] == true {
            return Err("Analysis remains pending; retry sync later".into());
        }
        self.db.record_attempt(&a, &p.problem, &feedback)?;
        let feedback = self
            .db
            .existing_attempt(&a.id)?
            .ok_or("Saved attempt missing")?
            .1;
        Ok(json!({"id":a.id,"feedback":feedback}))
    }
    pub fn create_job(
        &self,
        id: String,
        requested: usize,
        size: String,
        speed: String,
    ) -> Result<PackJob> {
        if !valid_id(&id)
            || requested == 0
            || requested > 50
            || !["mixed", "9", "13", "19"].contains(&size.as_str())
            || !["live", "realtime", "all", "correspondence"].contains(&speed.as_str())
        {
            return Err("Invalid pack settings; choose 1–50 problems".into());
        }
        if let Some(j) = self.db.job(&id)? {
            if j.requested != requested || j.size != size || j.speed != speed {
                return Err("Pack ID already belongs to different settings".into());
            }
            return Ok(j);
        }
        if self.db.unfinished_jobs()?.len() >= 4 {
            return Err("The preparation queue is full; wait for a pack to finish".into());
        }
        self.choose_game(&size, &speed)?;
        let j = PackJob {
            id,
            requested,
            size,
            speed,
            state: "queued".into(),
            completed: 0,
            error: None,
            current_moves_done: 0,
            current_moves_total: 0,
        };
        self.db.save_job(&j)?;
        Ok(j)
    }
    pub async fn run_job(&self, mut job: PackJob) -> Result<()> {
        job.state = "running".into();
        job.error = None;
        if !self.db.save_job_progress(&job)? {
            return Ok(());
        }
        while job.completed < job.requested {
            if self
                .db
                .job(&job.id)?
                .is_some_and(|j| j.state == "cancelled")
            {
                return Ok(());
            }
            let items = self.db.pack_problems(&job.id)?;
            let mut current = if let Some(p) = items.get(job.completed) {
                if p.problem.analysis.model != self.engine.identity && !p.problem.analysis.complete
                {
                    let updated = self.prepare(p.game.clone(), p.problem.turn).await?;
                    self.db
                        .link_problem(&job.id, job.completed, &updated.problem.id)?;
                    updated
                } else {
                    p.clone()
                }
            } else {
                let p = self.new_problem(&job.size, &job.speed).await?;
                self.db
                    .link_problem(&job.id, job.completed, &p.problem.id)?;
                p
            };
            job.current_moves_total = current.problem.legal_moves.len();
            job.current_moves_done = current.problem.analysis.candidates.len();
            if !self.db.save_job_progress(&job)? {
                return Ok(());
            }
            for m in current.problem.legal_moves.clone() {
                if self
                    .db
                    .job(&job.id)?
                    .is_some_and(|j| j.state == "cancelled")
                {
                    return Ok(());
                }
                {
                    let _lock = self.problem_lock.lock().await;
                    // Reload under the update lock so online answers and pack work cannot overwrite one another.
                    current = self
                        .db
                        .problem(&current.problem.id)?
                        .ok_or("Prepared problem disappeared")?;
                    self.evaluate_move(&mut current, &m).await?;
                }
                job.current_moves_done = current.problem.analysis.candidates.len();
                if !self.db.save_job_progress(&job)? {
                    return Ok(());
                }
            }
            if !current.problem.analysis.complete {
                return Err("Problem is incomplete; no offline-ready pack was published".into());
            }
            job.completed += 1;
            job.current_moves_done = 0;
            job.current_moves_total = 0;
            if !self.db.save_job_progress(&job)? {
                return Ok(());
            }
        }
        job.state = "ready".into();
        self.db.save_job_progress(&job)?;
        Ok(())
    }
    pub async fn worker(self: Arc<Self>) {
        loop {
            match self.db.unfinished_jobs() {
                Ok(jobs) => {
                    if let Some(job) = jobs.into_iter().next() {
                        let id = job.id.clone();
                        if let Err(e) = self.run_job(job).await {
                            eprintln!("Pack {id}: {e}");
                            if let Ok(Some(mut j)) = self.db.job(&id) {
                                if ["queued", "running"].contains(&j.state.as_str()) {
                                    j.state = "failed".into();
                                    j.error = Some(e);
                                    let _ = self.db.save_job_progress(&j);
                                }
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Pack queue: {e}"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}
fn move_eval(v: &Value) -> Result<MoveEval> {
    Ok(MoveEval {
        vertex: v["move"].as_str().ok_or("Missing move")?.into(),
        score: v["scoreLead"]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or("Missing candidate score")?,
        visits: v["visits"].as_u64().ok_or("Missing visit count")?,
        pv: serde_json::from_value(v["pv"].clone()).unwrap_or_default(),
    })
}
