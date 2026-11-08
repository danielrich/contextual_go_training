"""Opt-in real-engine integration test. Uses a disposable progress database."""
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import tempfile
from corpus import samples
from engine import Engine
from go_core import to_play
from server import Trainer
from store import Store

with tempfile.TemporaryDirectory() as d:
    store=Store(Path(d)/'smoke.sqlite'); engine=Engine(store); trainer=Trainer(store,engine)
    try:
        for size in [9,13,19]:
            position=trainer.new(dict(size=size,rank='10k'))
            session=trainer.sessions[position['id']]
            game,turn=session['game'],session['turn']
            analysis=engine.analyze(game,turn)
            best=[m['move'] for m in analysis['moveInfos'] if m['move']!='pass'][:3]
            assert len(best)==3
            score=analysis['rootInfo']['scoreLead']
            winner='even' if abs(score)<=2 else 'B' if score>0 else 'W'
            answers=[('winner',winner),('score',score),('groups',{str(g['id']):'uncertain' for g in position['groups']}),('candidates',best)]
            for stage,answer in answers:
                result=trainer.answer(dict(session=position['id'],stage=stage,answer=answer))
            assert len(result['candidates'])==3 and result['next_stage'] is None
            print(f'{size}x{size}: 4-stage session complete; score {score:.1f}; candidates '+str([(m['move'],m['loss']) for m in result['candidates']]),flush=True)
            # Also exercise White-to-play requests using a different turn.
            white_turn=turn if to_play(game,turn)=='W' else turn-1
            white=engine.analyze(game,white_turn)
            move=white['moveInfos'][0]['move']
            forced=engine.analyze(game,white_turn,forced=move)
            assert any(m['move']==move for m in forced['moveInfos'])
            print(f'{size}x{size}: White-to-play forced search verified',flush=True)
        assert store.progress()['total']==12
        print('PASS: real KataGo, all board sizes, both player perspectives, durable attempts.',flush=True)
    finally: engine.close()
