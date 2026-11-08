"""Bounded, persistent KataGo analysis process. Never substitutes fake evaluations."""
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import threading
import uuid
from go_core import to_play

ROOT=Path(__file__).resolve().parent

class Engine:
    def __init__(self,store):
        self.store=store; self.lock=threading.Lock(); self.proc=None; self.lines=None
        self.model=Path(os.environ.get('KATAGO_MODEL','/opt/homebrew/share/katago/g170e-b20c256x2-s5303129600-d1228401921.bin.gz'))
        self.binary=os.environ.get('KATAGO_BIN',shutil.which('katago') or 'katago')
        self.visits=int(os.environ.get('KATAGO_VISITS','200'))
        self.error=None
    def start(self):
        if self.proc and self.proc.poll() is None: return
        if not self.model.is_file(): raise RuntimeError('KataGo model not found. Set KATAGO_MODEL to a model file.')
        data=ROOT/'data'; data.mkdir(exist_ok=True)
        config=data/'analysis.cfg'
        config.write_text('''logDir = data/engine-logs
reportAnalysisWinratesAs = BLACK
numAnalysisThreads = 1
numSearchThreadsPerAnalysisThread = 4
nnMaxBatchSize = 8
nnCacheSizePowerOfTwo = 18
nnMutexPoolSizePowerOfTwo = 12
homeDataDir = data/katago-home
''')
        self.lines=queue.Queue(); lines=self.lines
        self.log=open(data/'engine-stderr.log','a')
        self.proc=subprocess.Popen([self.binary,'analysis','-config',str(config),'-model',str(self.model)],
                                   cwd=ROOT,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.log,text=True,bufsize=1)
        proc=self.proc
        def read():
            for line in proc.stdout:
                try: lines.put(json.loads(line))
                except json.JSONDecodeError: continue
            lines.put({'processEnded':True})
        threading.Thread(target=read,daemon=True).start()
    def close(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try: self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired: self.proc.kill(); self.proc.wait()
        if hasattr(self,'log'): self.log.close()
        self.proc=None
    def analyze(self,game,turn,forced=None):
        request=dict(moves=game['moves'][:turn],initialStones=game['setup'],initialPlayer=game['initial_player'],
                     rules=game['rules'],komi=game['komi'],boardXSize=game['size'],boardYSize=game['size'],
                     analyzeTurns=[turn],maxVisits=self.visits,includeOwnership=True)
        if forced: request['allowMoves']=[dict(player=to_play(game,turn),moves=[forced],untilDepth=1)]
        model_signature=f'{self.model}:{self.model.stat().st_size}:{self.model.stat().st_mtime_ns}' if self.model.exists() else str(self.model)
        key=hashlib.sha256((model_signature+json.dumps(request,sort_keys=True)).encode()).hexdigest()
        cached=self.store.cached(key)
        if cached: return cached
        with self.lock:
            cached=self.store.cached(key)
            if cached: return cached
            try:
                self.start(); ident=uuid.uuid4().hex; request['id']=ident
                self.proc.stdin.write(json.dumps(request)+'\n'); self.proc.stdin.flush()
                while True:
                    result=self.lines.get(timeout=120)
                    if result.get('processEnded'): raise RuntimeError('KataGo exited. See data/engine-stderr.log.')
                    if result.get('id')!=ident: continue
                    if 'error' in result: raise RuntimeError('KataGo: '+str(result['error']))
                    if result.get('isDuringSearch'): continue
                    if 'rootInfo' not in result or 'ownership' not in result: raise RuntimeError('Incomplete engine analysis.')
                    if forced and not any(m['move']==forced for m in result.get('moveInfos',[])):
                        raise RuntimeError('The engine did not evaluate this candidate.')
                    self.store.cache(key,result); self.error=None
                    return result
            except (queue.Empty,OSError,RuntimeError) as exc:
                self.error=str(exc) or 'KataGo analysis timed out. Retry or inspect data/engine-stderr.log.'
                self.close()
                raise RuntimeError(self.error) from exc
