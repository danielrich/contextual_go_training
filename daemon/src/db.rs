use crate::model::{Attempt, Game, PackJob, Result, StoredProblem};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Clone)]
pub struct Db {
    pub path: PathBuf,
}
#[derive(Clone, Debug, Serialize)]
pub struct Cohort {
    pub size: usize,
    pub speed: String,
    pub count: usize,
}
impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(err)?;
        }
        let db = Self { path };
        let mut c = db.connect()?;
        c.execute_batch("PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS games(id TEXT PRIMARY KEY,game TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS cache(key TEXT PRIMARY KEY,result TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT);
        CREATE TABLE IF NOT EXISTS wb_cohorts(size INTEGER,speed TEXT,n INTEGER NOT NULL,PRIMARY KEY(size,speed));
        CREATE TABLE IF NOT EXISTS wb_slots(size INTEGER,speed TEXT,slot INTEGER,game_id TEXT UNIQUE,PRIMARY KEY(size,speed,slot));
        CREATE TABLE IF NOT EXISTS wb_raw(id TEXT PRIMARY KEY,raw TEXT NOT NULL,status TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS wb_imports(source TEXT PRIMARY KEY,line INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS wb_problems(id TEXT PRIMARY KEY,data TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS wb_jobs(id TEXT PRIMARY KEY,data TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS wb_items(job_id TEXT,ordinal INTEGER,problem_id TEXT,PRIMARY KEY(job_id,ordinal));
        CREATE TABLE IF NOT EXISTS wb_attempts(id TEXT PRIMARY KEY,data TEXT NOT NULL,feedback TEXT NOT NULL,size INTEGER,black_rank TEXT,white_rank TEXT,stage TEXT,correct INTEGER,created INTEGER);
        CREATE INDEX IF NOT EXISTS wb_attempt_groups ON wb_attempts(stage,size,black_rank,white_rank);
        CREATE UNIQUE INDEX IF NOT EXISTS games_source_url ON games(json_extract(game,'$.source_url')) WHERE json_extract(game,'$.source_url') IS NOT NULL AND json_extract(game,'$.source_url') != '';").map_err(err)?;
        let initialized: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key='wb_slots_v1'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if initialized.is_none() {
            let tx = c.transaction().map_err(err)?;
            tx.execute_batch("INSERT OR IGNORE INTO wb_slots(size,speed,slot,game_id)
                SELECT json_extract(game,'$.size'),COALESCE(json_extract(game,'$.speed'),'unknown'),
                ROW_NUMBER() OVER(PARTITION BY json_extract(game,'$.size'),COALESCE(json_extract(game,'$.speed'),'unknown') ORDER BY rowid)-1,id FROM games;
                INSERT OR REPLACE INTO wb_cohorts SELECT size,speed,count(*) FROM wb_slots GROUP BY size,speed;
                INSERT INTO settings(key,value) VALUES ('wb_slots_v1','1');").map_err(err)?;
            tx.commit().map_err(err)?;
        }
        let has_prototype: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='attempts')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        let migrated: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key='wb_prototype_attempts_v1'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        if has_prototype && migrated.is_none() {
            let tx = c.transaction().map_err(err)?;
            tx.execute_batch("INSERT OR IGNORE INTO wb_attempts
                SELECT 'legacy-'||session||'-'||stage,
                json_object('id','legacy-'||session||'-'||stage,'problem_id','legacy-'||session,
                    'stage',stage,'answer',json_extract(safe_detail,'$.answer'),
                    'reasoning',COALESCE(json_extract(safe_detail,'$.reasoning'),''),
                    'created',CAST(COALESCE(strftime('%s',created),strftime('%s','now')) AS INTEGER),
                    'grader_version','prototype-v1'),
                COALESCE(json_extract(safe_detail,'$.feedback'),json_object('stage',stage,'correct',json(CASE WHEN correct THEN 'true' ELSE 'false' END))),
                size,
                CASE WHEN instr(rank,'/')>0 THEN substr(rank,1,instr(rank,'/')-1) ELSE COALESCE(rank,'Unknown') END,
                CASE WHEN instr(rank,'/')>0 THEN substr(rank,instr(rank,'/')+1) ELSE COALESCE(rank,'Unknown') END,
                stage,correct,CAST(COALESCE(strftime('%s',created),strftime('%s','now')) AS INTEGER)
                FROM (SELECT *,CASE WHEN json_valid(detail) THEN detail ELSE '{}' END AS safe_detail FROM attempts);
                INSERT INTO settings(key,value) VALUES ('wb_prototype_attempts_v1','1');").map_err(err)?;
            tx.commit().map_err(err)?;
        }
        Ok(db)
    }
    pub fn connect(&self) -> Result<Connection> {
        let c = Connection::open(&self.path).map_err(err)?;
        c.busy_timeout(Duration::from_secs(30)).map_err(err)?;
        Ok(c)
    }
    fn insert_game(tx: &Transaction<'_>, g: &Game) -> Result<bool> {
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO games(id,game) VALUES (?,?)",
                params![g.id, serde_json::to_string(g).map_err(err)?],
            )
            .map_err(err)?;
        if inserted == 0 {
            return Ok(false);
        }
        let slot: i64 = tx
            .query_row(
                "SELECT n FROM wb_cohorts WHERE size=? AND speed=?",
                params![g.size, g.speed],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?
            .unwrap_or(0);
        tx.execute(
            "INSERT INTO wb_slots VALUES (?,?,?,?)",
            params![g.size, g.speed, slot, g.id],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO wb_cohorts VALUES (?,?,1) ON CONFLICT(size,speed) DO UPDATE SET n=n+1",
            params![g.size, g.speed],
        )
        .map_err(err)?;
        Ok(true)
    }
    pub fn add_game(&self, g: &Game) -> Result<bool> {
        let mut c = self.connect()?;
        let tx = c.transaction().map_err(err)?;
        let result = Self::insert_game(&tx, g)?;
        tx.commit().map_err(err)?;
        Ok(result)
    }
    pub fn cohorts(&self) -> Result<Vec<Cohort>> {
        let c = self.connect()?;
        let mut s = c
            .prepare("SELECT size,speed,n FROM wb_cohorts ORDER BY size,speed")
            .map_err(err)?;
        let rows = s
            .query_map([], |r| {
                Ok(Cohort {
                    size: r.get(0)?,
                    speed: r.get(1)?,
                    count: r.get(2)?,
                })
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)
    }
    pub fn game_at(&self, size: usize, speed: &str, slot: usize) -> Result<Option<Game>> {
        let c = self.connect()?;
        let data:Option<String>=c.query_row("SELECT g.game FROM wb_slots s JOIN games g ON s.game_id=g.id WHERE s.size=? AND s.speed=? AND s.slot=?",params![size,speed,slot],|r|r.get(0)).optional().map_err(err)?;
        data.map(|s| serde_json::from_str(&s).map_err(err))
            .transpose()
    }
    pub fn import_cursor(&self, source: &str) -> Result<u64> {
        self.connect()?
            .query_row(
                "SELECT line FROM wb_imports WHERE source=?",
                [source],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)
            .map(|v| v.unwrap_or(0))
    }
    pub fn raw_count(&self) -> Result<u64> {
        self.connect()?
            .query_row(
                "SELECT value FROM settings WHERE key='wb_raw_count'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(err)
            .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }
    pub fn import_batch(
        &self,
        source: &str,
        rows: &[(String, String, Option<Game>, String)],
        line: u64,
    ) -> Result<()> {
        let mut c = self.connect()?;
        let tx = c.transaction().map_err(err)?;
        let mut added = 0;
        for (id, raw, g, status) in rows {
            added += tx
                .execute(
                    "INSERT OR IGNORE INTO wb_raw VALUES (?,?,?)",
                    params![id, raw, status],
                )
                .map_err(err)?;
            if let Some(g) = g {
                Self::insert_game(&tx, g)?;
            }
        }
        tx.execute("INSERT INTO settings VALUES ('wb_raw_count',?) ON CONFLICT(key) DO UPDATE SET value=CAST(value AS INTEGER)+CAST(excluded.value AS INTEGER)",[added.to_string()]).map_err(err)?;
        tx.execute("INSERT INTO wb_imports VALUES (?,?) ON CONFLICT(source) DO UPDATE SET line=excluded.line",params![source,line]).map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn cached(&self, key: &str) -> Result<Option<Value>> {
        let c = self.connect()?;
        let v: Option<String> = c
            .query_row("SELECT result FROM cache WHERE key=?", [key], |r| r.get(0))
            .optional()
            .map_err(err)?;
        v.map(|s| serde_json::from_str(&s).map_err(err)).transpose()
    }
    pub fn cache(&self, key: &str, v: &Value) -> Result<()> {
        self.connect()?
            .execute(
                "INSERT OR REPLACE INTO cache VALUES (?,?)",
                params![key, v.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn save_problem(&self, p: &StoredProblem) -> Result<()> {
        self.connect()?
            .execute(
                "INSERT OR REPLACE INTO wb_problems VALUES (?,?)",
                params![p.problem.id, serde_json::to_string(p).map_err(err)?],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn problem(&self, id: &str) -> Result<Option<StoredProblem>> {
        let c = self.connect()?;
        let v: Option<String> = c
            .query_row("SELECT data FROM wb_problems WHERE id=?", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        v.map(|s| serde_json::from_str(&s).map_err(err)).transpose()
    }
    pub fn save_job(&self, j: &PackJob) -> Result<()> {
        self.connect()?
            .execute(
                "INSERT OR REPLACE INTO wb_jobs VALUES (?,?)",
                params![j.id, serde_json::to_string(j).map_err(err)?],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn save_job_progress(&self, j: &PackJob) -> Result<bool> {
        let changed=self.connect()?.execute("UPDATE wb_jobs SET data=? WHERE id=? AND json_extract(data,'$.state') IN ('queued','running')",params![serde_json::to_string(j).map_err(err)?,j.id]).map_err(err)?;
        Ok(changed > 0)
    }
    pub fn job(&self, id: &str) -> Result<Option<PackJob>> {
        let c = self.connect()?;
        let v: Option<String> = c
            .query_row("SELECT data FROM wb_jobs WHERE id=?", [id], |r| r.get(0))
            .optional()
            .map_err(err)?;
        v.map(|s| serde_json::from_str(&s).map_err(err)).transpose()
    }
    pub fn unfinished_jobs(&self) -> Result<Vec<PackJob>> {
        let c = self.connect()?;
        let mut s=c.prepare("SELECT data FROM wb_jobs WHERE json_extract(data,'$.state') IN ('queued','running') ORDER BY rowid").map_err(err)?;
        let rows = s.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn jobs(&self) -> Result<Vec<PackJob>> {
        let c = self.connect()?;
        let mut s = c
            .prepare("SELECT data FROM wb_jobs ORDER BY rowid DESC LIMIT 50")
            .map_err(err)?;
        let rows = s.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn link_problem(&self, job: &str, ordinal: usize, id: &str) -> Result<()> {
        self.connect()?
            .execute(
                "INSERT OR REPLACE INTO wb_items VALUES (?,?,?)",
                params![job, ordinal, id],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn pack_problems(&self, job: &str) -> Result<Vec<StoredProblem>> {
        let c = self.connect()?;
        let mut s=c.prepare("SELECT p.data FROM wb_items i JOIN wb_problems p ON p.id=i.problem_id WHERE i.job_id=? ORDER BY i.ordinal").map_err(err)?;
        let rows = s.query_map([job], |r| r.get::<_, String>(0)).map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn existing_attempt(&self, id: &str) -> Result<Option<(Attempt, Value)>> {
        let c = self.connect()?;
        let v: Option<(String, String)> = c
            .query_row(
                "SELECT data,feedback FROM wb_attempts WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        v.map(|(a, f)| {
            Ok((
                serde_json::from_str(&a).map_err(err)?,
                serde_json::from_str(&f).map_err(err)?,
            ))
        })
        .transpose()
    }
    pub fn record_attempt(
        &self,
        a: &Attempt,
        p: &crate::model::Problem,
        feedback: &Value,
    ) -> Result<()> {
        let data = serde_json::to_string(a).map_err(err)?;
        let mut c = self.connect()?;
        let tx = c.transaction().map_err(err)?;
        let existing: Option<String> = tx
            .query_row("SELECT data FROM wb_attempts WHERE id=?", [&a.id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        if let Some(old) = existing {
            if old != data {
                return Err("Attempt ID already belongs to different content".into());
            }
            return Ok(());
        }
        tx.execute(
            "INSERT INTO wb_attempts VALUES (?,?,?,?,?,?,?,?,?)",
            params![
                a.id,
                data,
                feedback.to_string(),
                p.size,
                p.black_rank,
                p.white_rank,
                a.stage,
                feedback
                    .get("correct")
                    .and_then(Value::as_bool)
                    .map(i64::from),
                a.created
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }
    pub fn progress(&self) -> Result<Value> {
        let c = self.connect()?;
        let mut skills = json!({"winner":{"total":0,"correct":0},"score":{"total":0,"correct":0},"groups":{"total":0,"correct":0},"candidates":{"total":0,"correct":0}});
        let mut breakdown = vec![];
        let mut total = 0;
        let mut s=c.prepare("SELECT stage,size,black_rank,white_rank,count(*),sum(correct) FROM wb_attempts WHERE correct IS NOT NULL GROUP BY stage,size,black_rank,white_rank").map_err(err)?;
        let rows = s
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, usize>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, u64>(4)?,
                    r.get::<_, u64>(5)?,
                ))
            })
            .map_err(err)?;
        for row in rows {
            let (stage, size, b, w, n, correct) = row.map_err(err)?;
            if let Some(skill) = skills.get_mut(&stage) {
                skill["total"] = json!(skill["total"].as_u64().unwrap_or(0) + n);
                skill["correct"] = json!(skill["correct"].as_u64().unwrap_or(0) + correct);
            }
            total += n;
            breakdown.push(json!({"stage":stage,"size":size,"black_rank":b,"white_rank":w,"rank":format!("{b}/{w}"),"total":n,"correct":correct}));
        }
        let mut s=c.prepare("SELECT stage,size,correct,created FROM wb_attempts WHERE correct IS NOT NULL ORDER BY created DESC,rowid DESC LIMIT 12").map_err(err)?;
        let recent=s.query_map([],|r|Ok(json!({"stage":r.get::<_,String>(0)?,"size":r.get::<_,usize>(1)?,"correct":r.get::<_,bool>(2)?,"created":r.get::<_,u64>(3)?}))).map_err(err)?.collect::<std::result::Result<Vec<_>,_>>().map_err(err)?;
        let mut q = json!({"total_moves":0,"total_loss":0.0,"sound_moves":0,"average_loss":null,"breakdown":[]});
        let mut groups = vec![];
        let mut query=c.prepare("SELECT a.size,a.black_rank,a.white_rank,count(*),sum(json_extract(m.value,'$.loss')),sum(CASE WHEN json_extract(m.value,'$.correct')=1 THEN 1 ELSE 0 END) FROM wb_attempts a,json_each(a.feedback,'$.candidates') m WHERE a.stage='candidates' AND a.correct IS NOT NULL AND COALESCE(json_extract(a.feedback,'$.pending'),0)=0 AND json_type(m.value,'$.loss') IN ('integer','real') AND json_extract(m.value,'$.loss')>=0 GROUP BY a.size,a.black_rank,a.white_rank ORDER BY a.size,a.black_rank,a.white_rank").map_err(err)?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, usize>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, u64>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, u64>(5)?,
                ))
            })
            .map_err(err)?;
        let (mut moves, mut loss, mut sound) = (0_u64, 0.0, 0_u64);
        for row in rows {
            let (size, b, w, n, l, c) = row.map_err(err)?;
            moves += n;
            loss += l;
            sound += c;
            groups.push(json!({"size":size,"black_rank":b,"white_rank":w,"rank":format!("{b}/{w}"),"total_moves":n,"total_loss":l,"sound_moves":c,"average_loss":l/n as f64}));
        }
        q["total_moves"] = json!(moves);
        q["total_loss"] = json!(loss);
        q["sound_moves"] = json!(sound);
        if moves > 0 {
            q["average_loss"] = json!(loss / moves as f64);
        }
        q["breakdown"] = json!(groups);
        Ok(
            json!({"total":total,"skills":skills,"breakdown":breakdown,"recent":recent,"candidate_quality":q}),
        )
    }
}
