import tempfile
import unittest
from pathlib import Path
from server import Trainer
from store import Store
from corpus import import_text
from go_core import parse_sgf, rank_number

class CountingEngine:
    """API gating tests do not require a GPU; numerical grading has separate tests."""
    model=Path('/missing')
    error=None
    calls=0
    def analyze(self,game,turn,forced=None):
        self.calls+=1
        return {'rootInfo':{'scoreLead':5},'ownership':[.1]*(game['size']**2),
                'moveInfos':[dict(move=forced or 'A1',scoreLead=4,visits=200,pv=[forced or 'A1'])]}

class ApiTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(); self.store=Store(Path(self.tmp.name)/'test.db')
        self.engine=CountingEngine(); self.trainer=Trainer(self.store,self.engine)
    def tearDown(self): self.tmp.cleanup()
    def test_position_does_not_leak_analysis(self):
        position=self.trainer.new({'size':'mixed','rank':'10k'})
        self.assertEqual(position['size'],9)
        self.assertTrue(position['sample'])
        self.assertNotIn('ownership',position);self.assertNotIn('score',position)
        self.assertEqual(self.engine.calls,0)
    def test_question_gating_and_idempotency(self):
        p=self.trainer.new({'size':9,'rank':'10k'})
        with self.assertRaises(ValueError): self.trainer.answer({'session':p['id'],'stage':'score','answer':5})
        a={'session':p['id'],'stage':'winner','answer':'B'}
        first=self.trainer.answer(a);second=self.trainer.answer(a)
        self.assertEqual(first,second);self.assertEqual(self.engine.calls,1)
        self.assertNotIn('score',first)
        self.assertEqual(self.store.progress()['total'],1)
        for invalid in [float('nan'),float('inf'),True,'five']:
            with self.assertRaises(ValueError): self.trainer.answer({'session':p['id'],'stage':'score','answer':invalid})
    def test_full_session_tracks_all_skills(self):
        p=self.trainer.new({'size':9,'rank':'10k'})
        for stage,answer in [('winner','B'),('score',5),('groups',{str(g['id']):'uncertain' for g in p['groups']}),('candidates',['A9','B9','C9'])]:
            result=self.trainer.answer({'session':p['id'],'stage':stage,'answer':answer})
            self.assertTrue(result['correct'])
        self.assertIsNone(result['next_stage']);self.assertEqual(self.store.progress()['total'],4)
    def test_import_rejects_unknown_speed(self):
        with self.assertRaises(ValueError): import_text(self.store,'(;SZ[9];B[aa])','test')
        self.assertEqual(len(self.store.games()),0)
    def test_rank_does_not_restrict_the_selection_pool(self):
        from corpus import samples
        from unittest.mock import patch
        near=samples()[0];near.update(id='near',sample=False,black_rank='10k',white_rank='9k')
        far=dict(near,id='far',black_rank='1d',white_rank='2d')
        self.store.add_game(near);self.store.add_game(far)
        with patch('server.random.choice',side_effect=lambda pool: next(g for g in pool if g['id']=='far')):
            p=self.trainer.new({'size':9,'rank':'10k'})
        self.assertEqual(p['black_rank'],'1d')
        self.trainer.answer({'session':p['id'],'stage':'winner','answer':'B'})
        self.assertEqual(self.store.progress()['breakdown'][0]['rank'],'1d/2d')
    def test_latest_nonterminal_position_is_available_on_every_size(self):
        from corpus import samples
        from unittest.mock import patch
        for g in samples():
            with self.subTest(size=g['size']):
                g.update(id='late-'+str(g['size']),sample=False)
                from go_core import replay,play,vertex
                board=replay(g,len(g['moves']))
                old_cap={9:65,13:110,19:190}[g['size']]
                # Black passes while White legally fills new points, creating a
                # reproducible history beyond each former move-number cap.
                while len(g['moves'])<=old_cap:
                    for y in range(g['size']):
                        for x in range(g['size']):
                            move=vertex(x,y,g['size'])
                            try: next_board=play(board,'W',move)
                            except ValueError: continue
                            break
                        else: continue
                        break
                    g['moves'].extend([['B','pass'],['W',move]])
                    board=next_board
                replay(g,len(g['moves']))
                expected=len(g['moves'])
                self.store.add_game(g)
                with patch('server.random.randint',side_effect=lambda low,high:high):
                    p=self.trainer.new({'size':g['size'],'rank':'10k'})
                self.assertEqual(p['turn'],expected)
    def test_two_passes_are_the_game_boundary_not_a_move_cutoff(self):
        from corpus import samples
        from unittest.mock import patch
        g=samples()[0];g.update(id='finished',sample=False)
        original=len(g['moves']);g['moves'] += [['B','pass'],['W','pass']]
        self.store.add_game(g)
        with patch('server.random.randint',side_effect=lambda low,high:high):
            p=self.trainer.new({'size':9,'rank':'10k'})
        self.assertEqual(p['turn'],original+1)
    def test_rank_parsing(self):
        self.assertEqual(rank_number('10 kyu'),-10)
        self.assertEqual(rank_number('3d'),2)
        self.assertIsNone(rank_number('Unknown'))
