import unittest
from go_core import parse_sgf, replay, play, eligible, coord, group_at

class CoreTests(unittest.TestCase):
    def test_mainline_ignores_variations_and_escaped_comments(self):
        g=parse_sgf(r'(;SZ[9]KM[6.5]RU[Chinese]TM[600]OT[5x30 byo-yomi]BR[10k]WR[9k]C[escaped\]text];B[aa];W[ba](;B[cc])(;B[dd]))')
        self.assertEqual(g['moves'],[['B','A9'],['W','B9'],['B','C7']])
        self.assertEqual(g['komi'],6.5)
        self.assertTrue(eligible(g))
    def test_correspondence_and_unknown_are_excluded(self):
        self.assertFalse(eligible(parse_sgf('(;SZ[19]TM[259200];B[aa])')))
        self.assertFalse(eligible(parse_sgf('(;SZ[19];B[aa])')))
        self.assertFalse(eligible(parse_sgf('(;SZ[9]TM[600]OT[1/86400];B[aa])')))
    def test_setup_capture_and_pass(self):
        g=parse_sgf('(;SZ[9]AB[ab][ba][cb]AW[bb]PL[B];B[bc];W[])')
        b=replay(g,2)
        self.assertEqual(b[1][1],0)
        self.assertEqual(b[2][1],1)
        self.assertEqual(g['moves'][-1],['W','pass'])
    def test_illegal_and_suicide(self):
        b=[[0]*9 for _ in range(9)]
        b[0][1]=b[1][0]=2
        with self.assertRaises(ValueError): play(b,'B','A9')
        with self.assertRaises(ValueError): play(b,'B','B9')
        self.assertEqual(b[0][0],0)
    def test_coordinate_skips_i(self):
        self.assertEqual(coord('J10',19),(8,9))
        with self.assertRaises(ValueError): coord('I10',19)
    def test_connected_group(self):
        b=[[0]*9 for _ in range(9)]
        b[0][0]=b[0][1]=1
        self.assertEqual(len(group_at(b,0,0)[0]),2)
    def test_ko_replay_rejected(self):
        g=parse_sgf('(;SZ[9]AB[ab][ba][bc]AW[bb][ca][cc][db];B[cb];W[bb])')
        with self.assertRaises(ValueError): replay(g,2)
    def test_malformed_and_unsupported(self):
        for text in ['garbage','(;SZ[17];B[aa])','(;SZ[9];B[zz])','(;SZ[9];B[aa]']:
            with self.assertRaises(ValueError): parse_sgf(text)

class ImportMetadataTests(unittest.TestCase):
    def test_missing_komi_cannot_enter_verified_pool(self):
        game=parse_sgf('(;SZ[9]TM[600]RU[Chinese];B[cc])')
        self.assertFalse(eligible(game))
    def test_suicide_permitting_rules_are_not_accepted_as_supported(self):
        game=parse_sgf('(;SZ[9]KM[6.5]RU[Tromp-Taylor]TM[600];B[cc])')
        self.assertFalse(eligible(game))
    def test_missing_rules_cannot_enter_verified_pool(self):
        game=parse_sgf('(;SZ[9]KM[6.5]TM[600];B[cc])')
        self.assertFalse(eligible(game))

class ArchiveMetadataTests(unittest.TestCase):
    def test_explicit_correspondence_overrides_short_main_time(self):
        g=parse_sgf('(;SZ[9]KM[5.5]RU[Japanese]TM[300]GC[correspondence,ranked];B[cc])')
        self.assertFalse(eligible(g))
    def test_explicit_live_supports_zero_main_time_byoyomi(self):
        g=parse_sgf('(;SZ[9]KM[5.5]RU[Japanese]TM[0]OT[5x30 byo-yomi]GC[live,ranked]PC[OGS: https://online-go.com/game/123];B[cc])')
        self.assertTrue(eligible(g))
        self.assertEqual(g['source_url'],'https://online-go.com/game/123')
    def test_blitz_and_rapid_do_not_enter_live_only_archive_pool(self):
        for speed in ['blitz','rapid']:
            g=parse_sgf(f'(;SZ[9]KM[5.5]RU[Japanese]TM[300]GC[{speed},ranked];B[cc])')
            self.assertFalse(eligible(g))
