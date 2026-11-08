#!/usr/bin/env python3
"""Local whole-board Go trainer. Run: python3 server.py"""
import argparse
import atexit
import json
import math
from pathlib import Path
import random
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse
from corpus import samples, import_path, import_text
from engine import Engine
from go_core import replay, to_play, group_at, vertex, rank_number
from store import Store
from training import candidate_loss, winner_grade, score_grade, group_estimate, validate_candidates

ROOT=Path(__file__).resolve().parent

class Trainer:
    def __init__(self,store,engine):
        self.store=store; self.engine=engine; self.sessions={}; self.lock=threading.Lock(); self.intros=samples()
    def status(self):
        games=self.store.games()
        return dict(games=len(games),sizes={str(n):sum(g['size']==n for g in games) for n in [9,13,19]},
                    engine=Path(self.engine.model).is_file(),engine_error=self.engine.error,model=Path(self.engine.model).name,
                    progress=self.store.progress())
    def new(self,body):
        size=body.get('size','mixed'); rank=str(body.get('rank','10k'))
        r=rank_number(rank)
        if r is None or not -30<=r<=8: raise ValueError('Choose a rank from 30k to 9d.')
        if str(size) not in ['mixed','9','13','19']: raise ValueError('Unsupported board size.')
        # A first mixed session starts small. Afterwards, 19x19 dominates.
        size=(9 if self.store.progress()['total']==0 else random.choices([9,13,19],[2,2,6])[0]) if size=='mixed' else int(size)
        games=[g for g in self.store.games() if g['size']==size]
        g=random.choice(games) if games else next(g for g in self.intros if g['size']==size)
        upper=len(g['moves'])
        # Include late endgame and the first pass, but do not ask for a next move
        # after the two-pass game boundary (or sample later scoring edits).
        for i in range(1,len(g['moves'])):
            if g['moves'][i-1][1]=='pass' and g['moves'][i][1]=='pass':
                upper=i
                break
        lower=1
        turn=random.randint(lower,upper) if not g['sample'] else len(g['moves'])-2
        board=replay(g,turn); groups=[]; seen=set()
        for y,row in enumerate(board):
            for x,c in enumerate(row):
                if c and (x,y) not in seen:
                    stones,libs=group_at(board,x,y); seen.update(stones)
                    groups.append(dict(id=len(groups)+1,color='B' if c==1 else 'W',anchor=vertex(x,y,size),
                                       stones=[list(p) for p in sorted(stones)],liberties=len(libs)))
        ident=uuid.uuid4().hex
        with self.lock:
            if len(self.sessions)>=128: self.sessions.pop(next(iter(self.sessions)))
            self.sessions[ident]=dict(game=g,turn=turn,stage=0,groups=groups,results={},lock=threading.Lock())
        return dict(id=ident,size=size,board=board,turn=turn,to_play=to_play(g,turn),komi=g['komi'],rules=g['rules_label'],
                    sample=g['sample'],source=g['source'],black=g['black'],white=g['white'],black_rank=g['black_rank'],
                    white_rank=g['white_rank'],groups=groups,date=g.get('date',''),
                    source_url=g.get('source_url',''),
                    last_move=g['moves'][turn-1][1] if turn else None)
    def answer(self,body):
        ident=body.get('session'); session=self.sessions.get(ident)
        if not session: raise ValueError('This session expired. Start a new position.')
        with session['lock']: return self._answer(ident,session,body)
    def _answer(self,ident,s,body):
        stages=['winner','score','groups','candidates']; stage=body.get('stage')
        if stage in s['results']: return s['results'][stage]
        if s['stage']>=4 or stage!=stages[s['stage']]: raise ValueError('Complete the current question first.')
        answer=body.get('answer'); g=s['game']; turn=s['turn']; player=to_play(g,turn)
        if stage=='winner' and answer not in ['B','W','even']: raise ValueError('Choose Black, White or close.')
        if stage=='score' and (isinstance(answer,bool) or not isinstance(answer,(int,float)) or not math.isfinite(answer) or abs(answer)>g['size']**2+100):
            raise ValueError('Enter a finite score estimate in points.')
        if stage=='groups':
            if not isinstance(answer,dict) or any(str(group['id']) not in answer or answer[str(group['id'])] not in ['secure-looking','at-risk','uncertain'] for group in s['groups']):
                raise ValueError('Assess every marked group.')
        if stage=='candidates': validate_candidates(g,turn,answer)
        analysis=self.engine.analyze(g,turn); score=analysis['rootInfo']['scoreLead']
        result=dict(stage=stage)
        if stage=='winner':
            expected='even' if abs(score)<=2 else 'B' if score>0 else 'W'
            result.update(correct=winner_grade(answer,score),expected=expected,
                          message='Close means within 2 points. Now commit to a count before seeing the engine estimate.')
        elif stage=='score':
            result.update(correct=score_grade(answer,score),score=round(score,1),error=round(abs(answer-score),1),
                          message='KataGo estimates the final lead with strong play, including komi. This is not just settled territory; ±3 points earns credit.')
        elif stage=='groups':
            estimates=[]
            for group in s['groups']:
                values=[analysis['ownership'][y*g['size']+x] for x,y in group['stones']]
                expected=group_estimate(values,group['color'])
                estimates.append(dict(id=group['id'],anchor=group['anchor'],expected=expected,answer=answer[str(group['id'])],
                                      correct=answer[str(group['id'])]==expected))
            result.update(correct=all(v['correct'] for v in estimates),groups=estimates,
                          message='Ownership agreement only: “secure-looking” is ≥90% own-color ownership at every stone; “at-risk” is ≥70% opponent ownership at every stone. Everything else is uncertain. These are not proofs of life, death or seki.')
        else:
            candidates=[]
            for move in answer:
                forced=self.engine.analyze(g,turn,forced=move)
                info=next(m for m in forced['moveInfos'] if m['move']==move)
                loss=candidate_loss(score,info['scoreLead'],player)
                candidates.append(dict(move=move,loss=round(loss,1),correct=loss<=2,pv=info.get('pv',[])[:7],visits=info['visits']))
            result.update(correct=all(v['correct'] for v in candidates),candidates=candidates,
                          suggestions=[dict(move=m['move'],pv=m.get('pv',[])[:7]) for m in analysis['moveInfos'][:3]],
                          message='Each candidate is independently searched from the same position. Within 2 estimated points of the baseline earns credit. Search noise can affect close calls.')
        result['next_stage']=stages[s['stage']+1] if s['stage']<3 else None
        source_rank=g['black_rank']+'/'+g['white_rank'] if not g['sample'] else 'introductory'
        self.store.record(session=ident,stage=stage,size=g['size'],rank=source_rank,correct=result['correct'],detail=dict(answer=answer,feedback=result,source=g['id'],turn=turn))
        s['results'][stage]=result; s['stage']+=1
        return result


def make_handler(trainer):
    class Handler(BaseHTTPRequestHandler):
        def send_json(self,data,status=200):
            raw=json.dumps(data,allow_nan=False).encode(); self.send_response(status)
            self.send_header('Content-Type','application/json'); self.send_header('Content-Length',str(len(raw)))
            self.send_header('Cache-Control','no-store'); self.end_headers(); self.wfile.write(raw)
        def do_GET(self):
            path=urlparse(self.path).path
            if path=='/api/status': return self.send_json(trainer.status())
            files={'/':'index.html','/app.js':'app.js','/style.css':'style.css'}
            if path not in files: return self.send_error(404)
            file=ROOT/'static'/files[path]
            raw=file.read_bytes(); self.send_response(200)
            self.send_header('Content-Type',{'html':'text/html; charset=utf-8','js':'text/javascript; charset=utf-8','css':'text/css; charset=utf-8'}[file.suffix[1:]])
            self.send_header('Content-Length',str(len(raw))); self.send_header('Cache-Control','no-cache')
            self.send_header('X-Content-Type-Options','nosniff'); self.end_headers(); self.wfile.write(raw)
        def do_POST(self):
            # Local service: reject browser requests from other origins.
            origin=self.headers.get('Origin')
            if origin and origin not in [f'http://127.0.0.1:{self.server.server_port}',f'http://localhost:{self.server.server_port}']:
                return self.send_json({'error':'Cross-origin requests are not allowed.'},403)
            if self.headers.get_content_type()!='application/json': return self.send_json({'error':'Expected JSON.'},415)
            try:
                length=int(self.headers.get('Content-Length','0'))
                if not 0<length<=5_000_000: raise ValueError('Request must be under 5 MB.')
                body=json.loads(self.rfile.read(length))
                if not isinstance(body,dict): raise ValueError('Expected an object.')
                path=urlparse(self.path).path
                if path=='/api/new': result=trainer.new(body)
                elif path=='/api/answer': result=trainer.answer(body)
                elif path=='/api/import': result=dict(id=import_text(trainer.store,body.get('sgf',''),str(body.get('filename','Uploaded SGF'))[:200]))
                else: return self.send_error(404)
                self.send_json(result)
            except (ValueError,TypeError,KeyError) as e: self.send_json({'error':str(e)},400)
            except RuntimeError as e: self.send_json({'error':str(e)},503)
            except Exception:
                import traceback; traceback.print_exc()
                self.send_json({'error':'Unexpected server error. See the terminal for details.'},500)
    return Handler


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port',type=int,default=8765)
    parser.add_argument('--import-path',help='SGF file, directory, zip or tar archive (read-only)')
    parser.add_argument('--limit',type=int,default=5000)
    args=parser.parse_args(); store=Store(ROOT/'data/trainer.sqlite')
    if args.import_path:
        print(json.dumps(import_path(store,args.import_path,args.limit),indent=2)); return
    engine=Engine(store); atexit.register(engine.close)
    trainer=Trainer(store,engine)
    server=ThreadingHTTPServer(('127.0.0.1',args.port),make_handler(trainer))
    print(f'Whole Board is running at http://127.0.0.1:{args.port}',flush=True)
    try: server.serve_forever()
    except KeyboardInterrupt: pass
    finally: server.server_close(); engine.close()

if __name__=='__main__': main()
