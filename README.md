# Whole Board

A local Go training room: judge the winner, estimate the score, assess groups,
then propose three independent sound moves. Feedback comes from your installed
KataGo, with scores hidden until you commit. No account or JavaScript dependencies.

## Run

```sh
python3 server.py
```

Open **http://127.0.0.1:8765**. Stop with Ctrl-C. Python 3.10+ and KataGo are required.
On this Mac the installed Metal engine requires normal machine access; running
inside a restricted agent sandbox can prevent both GPU access and listening on a port.

The default model is the installed Homebrew 20-block model. Override settings:

```sh
KATAGO_MODEL=/absolute/path/to/model.bin.gz KATAGO_VISITS=500 python3 server.py --port 8765
```

`KATAGO_BIN` overrides the executable. Default search is 200 visits per position
or forced candidate; evaluations near a threshold may vary with deeper search.
The engine starts on the first submitted answer, which may take longer.

## Import the historical archive

The application never changes original files. Supported inputs: SGF file,
a directory of SGFs, a zip/tar archive containing SGFs, or a single gzip SGF.

```sh
python3 server.py --import-path /absolute/path/to/archive --limit 5000
```

Import individual files through **Game library** in the browser. Import requires
explicit komi, a supported ruleset (Japanese, Chinese, or AGA), no handicap,
and at least 12 legal moves. Archive speed labels in GC (live/correspondence/blitz/rapid) take precedence over
main-time heuristics. Only live is accepted; zero-main-time live byo-yomi games
are supported. Without a speed label, the conservative filter requires main time
between 1 second and 2 hours. Hourly/daily overtime and unknown controls are excluded. This checks SGF metadata; it does not
independently verify the archive's provenance, and may reject unusual live controls.

Example archive location on your server:
`/path/to/go-archive/raw_games/sgfs-by-username`.
A read-only copy is stored locally in ignored `data/ogs-archive.tar.gz`; the
original remote files are unchanged. Import report: `data/ogs-import-report.json`.
Of 23,444 source files, 13,684 distinct games were accepted (8,683 on 9×9; 1,089
on 13×13; 3,912 on 19×19), 7 were duplicate copies, and 9,753 failed eligibility
or replay checks. Both players are within three ranks of 10 kyu in 3,714 games.
Original OGS game URLs are preserved and linked in the training room.

To reproduce the local import:

```sh
python3 server.py --import-path data/ogs-archive.tar.gz --limit 30000
```

Imports skip games already stored. No SSH connection is needed for daily practice.
If no imported games exist for a board size, an explicitly labeled constructed
introductory position is available. CSV, database, JSON and zstd formats are not supported.

## Training behavior

- Mixed practice starts with 9×9; later selections use 20% 9×9, 20% 13×13, 60% 19×19.
- All recorded player ranks are included in selection. Accuracy remains separated
  by the original Black/White ranks, board size and question type. Your stated rank
  does not restrict the game pool.
- Positions can be selected throughout the game, including late endgame and the
  first pass. There are no move-number caps; the two-pass game boundary ends selection.
- Each position is static. Three candidates are alternatives from that same position,
  never a three-move sequence. Pass is a valid candidate, including in endgame practice. Every submitted
  candidate receives a forced root search.
- Winner: “close” means an estimated lead within 2 points. Score: within 3 points
  earns credit. Candidates: each within 2 estimated points of the baseline earns credit.
- Group assessment compares ownership estimates: secure-looking if every stone has
  ≥90% own-color ownership; at-risk if every stone has ≥70% opponent ownership;
  otherwise uncertain. These labels do **not** prove life, death, seki, or tactical safety.
- Progress counts complete question submissions by skill, board size, and source ranks.
  Group/candidate stage success currently requires all answers to meet the criterion;
  per-group and per-move results remain in the stored attempt detail.
- Rank is self-reported. Automatic rank estimation, adaptive difficulty, spaced
  repetition, and long-term rank prediction are not part of this first version.

## Data and limits

`data/trainer.sqlite` stores the imported corpus, engine cache and question attempts.
Copy this file for a backup while the server is stopped. Engine logs are in `data/`.
Reasoning notes are stored in this browser's localStorage; no notes review screen yet.
Incomplete sessions live in server memory and expire after a server restart or after
128 newer sessions. Completed answers remain in SQLite. The server binds localhost.
Fonts are fetched from Google Fonts when available; system fonts work offline.

Bulk import is limited to 5,000 accepted files per invocation by default. The initial
corpus selector loads imported games into memory; indexing a very large archive needs
a separate streaming ingestion/indexing milestone. SGF imports use the first variation
only, reject mid-game setup edits, suicide, and repeated board cycles. This intentionally
excludes unusual records, including some legal Japanese long-cycle games.

## Verify

```sh
python3 -m unittest discover -s tests
node --check static/app.js
python3 scripts/smoke_engine.py
python3 scripts/smoke_archive.py
```

The last check uses the real installed GPU engine and a temporary progress database:
full four-stage sessions on all three sizes plus White-to-play forced searches.


## Full JSON archive on the training server

The existing `get_all_the_games.sh` was rerun successfully on 2026-11-08.
The completed source is on `your-go-server.local` at:

```text
/path/to/go-archive/ogs_games_2013_to_2025-05.json.gz
```

Its size is 45,847,026,468 bytes, matching the download server's Content-Length.
The publisher lists approximately 56 million games in this release:
https://za3k.com/ogs/

This is the full raw JSON source; the earlier SGF copy contains only username
prefix shard `69`. Initial JSON records were parsed to confirm the format.
The current trainer still uses its imported SGF subset. A streaming, disk-backed
import/index is needed before using the full JSON corpus; do not pass millions
of records through the current in-memory `Store.games()` selector.

The proposed daemon/PWA deployment and offline practice-pack design is documented
in `docs/architecture/phone-offline.md`. It is a proposal, not a deployed service.
