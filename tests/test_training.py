import tempfile
import unittest
from pathlib import Path
from training import candidate_loss, winner_grade, score_grade, group_estimate, validate_candidates
from store import Store
from go_core import parse_sgf

class TrainingTests(unittest.TestCase):
    def test_loss_uses_player_perspective(self):
        self.assertAlmostEqual(candidate_loss(4,1,'B'),3)
        self.assertAlmostEqual(candidate_loss(-4,-1,'W'),3)
        self.assertEqual(candidate_loss(1,4,'B'),0)
    def test_winner_close_and_score_tolerance(self):
        self.assertTrue(winner_grade('even',1.8))
        self.assertFalse(winner_grade('B',-6))
        self.assertTrue(score_grade(-3,-4.5))
        self.assertFalse(score_grade(12,-4))
    def test_group_estimates_never_claim_life_proof(self):
        self.assertEqual(group_estimate([.99,.98], 'B'),'secure-looking')
        self.assertEqual(group_estimate([-.99,-.98], 'B'),'at-risk')
        self.assertEqual(group_estimate([.5,-.2], 'B'),'uncertain')
        self.assertEqual(group_estimate([-.99,-.98], 'W'),'secure-looking')
    def test_three_distinct_legal_moves_required(self):
        g=parse_sgf('(;SZ[9];B[aa];W[bb])')
        validate_candidates(g,2,['C3','D4','E5'])
        for moves in [['A9','D4','E5'],['C3','C3','E5'],['C3','D4']]:
            with self.assertRaises(ValueError): validate_candidates(g,2,moves)
    def test_attempt_idempotency_and_skill_metrics(self):
        with tempfile.TemporaryDirectory() as d:
            s=Store(Path(d)/'test.db')
            a=dict(session='s1',stage='winner',size=9,rank='10k',correct=True,detail={'answer':'B'})
            s.record(**a); s.record(**a)
            self.assertEqual(s.progress()['total'],1)
            s.record(session='s2',stage='winner',size=19,rank='9k',correct=False,detail={})
            p=s.progress()
            self.assertEqual(p['total'],2)
            self.assertEqual(p['skills']['winner']['correct'],1)
            self.assertEqual(len(p['breakdown']),2)

class StoreConnectionTests(unittest.TestCase):
    def test_connection_is_closed_after_operation(self):
        import sqlite3
        with tempfile.TemporaryDirectory() as folder:
            store=Store(Path(folder)/'test.sqlite')
            with store.connect() as connection:
                self.assertEqual(connection.execute('SELECT 1').fetchone()[0],1)
            with self.assertRaises(sqlite3.ProgrammingError):
                connection.execute('SELECT 1')

class EndgameCandidateTests(unittest.TestCase):
    def test_pass_is_a_legal_independent_candidate(self):
        game=parse_sgf('(;SZ[9];B[cc];W[gg])')
        validate_candidates(game,2,['pass','D4','E5'])
        with self.assertRaises(ValueError): validate_candidates(game,2,['pass','pass','E5'])
