"""Verify historical game selection and real engine grading without recording practice."""
import sys
import json
import sqlite3
import tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from engine import Engine
from server import Trainer
from store import Store
from unittest.mock import patch

root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as folder:
    store=Store(Path(folder)/'test.sqlite')
    with sqlite3.connect(f'file:{root / "data/trainer.sqlite"}?mode=ro',uri=True) as corpus:
        for size in [9,13,19]:
            row=corpus.execute('''SELECT game FROM games WHERE json_extract(game,'$.size')=?
                ORDER BY json_array_length(json_extract(game,'$.moves')) DESC LIMIT 1''',(size,)).fetchone()
            assert row, f'No historical {size}x{size} game'
            store.add_game(json.loads(row[0]))
    engine=Engine(store); trainer=Trainer(store,engine)
    try:
        for size in [9,13,19]:
            with patch('server.random.randint',side_effect=lambda low,high:high):
                p=trainer.new({'size':size,'rank':'10k'})
            assert not p['sample'] and p['source_url'].startswith('https://online-go.com/game/')
            a=trainer.answer({'session':p['id'],'stage':'winner','answer':'even'})
            assert a['next_stage']=='score'
            s=trainer.sessions[p['id']]
            analysis=engine.analyze(s['game'],s['turn'])
            move=analysis['moveInfos'][0]['move']
            candidate=engine.analyze(s['game'],s['turn'],forced=move)
            assert any(m['move']==move for m in candidate['moveInfos'])
            print(f'{size}x{size}: historical {p["black_rank"]}/{p["white_rank"]}, move {p["turn"]}, actual engine grading + forced {move}: PASS · {p["source_url"]}',flush=True)
    finally: engine.close()
print('PASS: original corpus read-only, user practice history unchanged.')
