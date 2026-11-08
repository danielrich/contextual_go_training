"""SQLite progress, corpus and cached analysis; one connection per operation."""
import json
import sqlite3
from contextlib import contextmanager
from pathlib import Path

class Store:
    def __init__(self,path):
        self.path=str(path); Path(path).parent.mkdir(parents=True,exist_ok=True)
        with self.connect() as db:
            db.executescript('''
            CREATE TABLE IF NOT EXISTS games (id TEXT PRIMARY KEY, game TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, result TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS attempts (session TEXT, stage TEXT, size INTEGER, rank TEXT, correct INTEGER, detail TEXT, created TEXT DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY(session,stage));
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY,value TEXT);
            ''')
    @contextmanager
    def connect(self):
        db=sqlite3.connect(self.path,timeout=30)
        try:
            with db: yield db
        finally: db.close()
    def add_game(self,g):
        with self.connect() as db: db.execute('INSERT OR IGNORE INTO games VALUES (?,?)',(g['id'],json.dumps(g)))
    def games(self):
        with self.connect() as db: return [json.loads(row[0]) for row in db.execute('SELECT game FROM games')]
    def game_ids(self):
        with self.connect() as db: return {row[0] for row in db.execute('SELECT id FROM games')}
    def cached(self,key):
        with self.connect() as db: row=db.execute('SELECT result FROM cache WHERE key=?',(key,)).fetchone()
        return json.loads(row[0]) if row else None
    def cache(self,key,result):
        with self.connect() as db: db.execute('INSERT OR REPLACE INTO cache VALUES (?,?)',(key,json.dumps(result)))
    def record(self,session,stage,size,rank,correct,detail):
        with self.connect() as db:
            db.execute('INSERT OR IGNORE INTO attempts(session,stage,size,rank,correct,detail) VALUES (?,?,?,?,?,?)',
                       (session,stage,size,rank,int(correct),json.dumps(detail)))
    def progress(self):
        with self.connect() as db:
            rows=db.execute('SELECT stage,size,rank,count(*),sum(correct) FROM attempts GROUP BY stage,size,rank').fetchall()
            recent=db.execute('SELECT stage,size,correct,created FROM attempts ORDER BY created DESC,rowid DESC LIMIT 12').fetchall()
        skills={s:dict(total=0,correct=0) for s in ['winner','score','groups','candidates']}
        for stage,size,rank,n,c in rows:
            skills[stage]['total']+=n; skills[stage]['correct']+=c
        return dict(total=sum(r[3] for r in rows),skills=skills,
                    breakdown=[dict(stage=r[0],size=r[1],rank=r[2],total=r[3],correct=r[4]) for r in rows],
                    recent=[dict(stage=r[0],size=r[1],correct=bool(r[2]),created=r[3]) for r in recent])
