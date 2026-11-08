import tempfile
import unittest
import zipfile
from pathlib import Path
from corpus import import_path
from store import Store

SGF='(;SZ[9]KM[5.5]RU[Japanese]TM[0]OT[5x30 byo-yomi]GC[live,ranked]PC[OGS: https://online-go.com/game/123];B[cc];W[gg];B[cg];W[gc];B[ee];W[eg];B[ce];W[ge];B[cd];W[gf];B[dc];W[fg])'

class ArchiveTests(unittest.TestCase):
    def test_duplicate_player_folders_count_one_game_and_exclude_correspondence(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder); archive=root/'players.zip'; store=Store(root/'test.sqlite')
            with zipfile.ZipFile(archive,'w') as z:
                z.writestr('black/123.sgf',SGF)
                z.writestr('white/123.sgf',SGF)
                z.writestr('white/456.sgf',SGF.replace('live,ranked','correspondence,ranked'))
            report=import_path(store,archive)
            self.assertEqual((report['imported'],report['duplicates'],report['skipped']),(1,1,1))
            self.assertEqual(len(store.games()),1)
            self.assertEqual(store.games()[0]['source_url'],'https://online-go.com/game/123')
            report=import_path(store,archive)
            self.assertEqual((report['imported'],report['duplicates'],report['skipped']),(0,2,1))
