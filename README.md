# Contextual Go Training — Whole Board

Whole-board Go practice from historical games: judge who is ahead, count the
score, assess groups, and propose sound candidate moves. KataGo supplies the
reference analysis. The phone app can download complete problem packs and grade
them offline, then sync answers and reasoning back to the server.

## What is implemented

- Rust/Axum daemon with SQLite storage and a persistent KataGo subprocess.
  The engine warms at startup and prepares a small live-position buffer so
  practice starts can reuse completed analysis. Buffer misses still need search.
- Installable PWA with an offline app shell, IndexedDB problem packs, resumable
  sessions, saved reasoning, and an idempotent attempt outbox.
- Background preparation of 1–50 problems per pack (25 by default). Each ready
  problem contains a searched evaluation for **every legal move, including pass**.
- Rank-independent selection on 9×9, 13×13 and 19×19. Mixed practice emphasizes
  19×19. All playable phases, including late endgame, can be selected.
- Accuracy grouped by question, board size, and the rounded mean game rank.
  Candidate progress also reports average point loss per proposed move, sound-move
  percentage and sample counts by board size and mean game rank.
- Hover or focus engine-line buttons to preview numbered continuations on the
  board. Tap/click to pin a line, step through moves, or restore the original board.
- Live, real-time, correspondence, and all-speed practice settings; handicap
  positions are supported when their setup and rules are available.
- Streaming JSONL/gzip import, persistent import checkpoints, a disk-backed
  training index, and retention of raw records that need further normalization.
- systemd unit, installer, release packager, health/assets verifier, and CI.

The service currently has one shared practice profile. The self-reported rank
is a profile setting; it does not restrict game selection or estimate OGS rank.
Archive player ranks are labeled as recorded snapshots, not independently
verified historical ranks at the time of play.

## Run locally

Requires Rust (tested with 1.92), a C compiler, and an installed KataGo executable
and model. Node 22+ is needed only for browser-module tests; the deployed service
does not require Node or Python.

```sh
cargo build --locked
WB_KATAGO_BIN=/path/to/katago \
WB_KATAGO_MODEL=/path/to/model.bin.gz \
cargo run --locked -- serve --port 8765
```

Open [http://127.0.0.1:8765](http://127.0.0.1:8765). The default database is
`data/trainer.sqlite`. An existing prototype database at that path is indexed on
first Rust startup. Import a corpus before requesting a problem.

## Index the full OGS archive

The public archive is available from [za3k's OGS collection](https://za3k.com/ogs/).
The 2025 JSON release is approximately 45.8 GB compressed. The importer reads one
record at a time and commits in batches; it never loads the whole corpus into RAM.

```sh
# A bounded initial import, so you can inspect the results first.
target/debug/whole-board import-json \
  --input /path/to/ogs_games_2013_to_2025-05.json.gz \
  --db data/trainer.sqlite --limit 50000

# Continue from the committed checkpoint, with no record-count limit.
target/debug/whole-board import-json \
  --input /path/to/ogs_games_2013_to_2025-05.json.gz \
  --db data/trainer.sqlite --limit 0
```

Re-running the same command resumes without duplicating records. A gzip resume
must decompress the already-read prefix before reaching the checkpoint; the
stored records are not reprocessed. Source identity includes path, size and
modification time, so keep the source immutable. A full import materializes raw
JSON and normalized games in SQLite and requires substantially more disk space
than the compressed archive.

Unsupported boards/rules or malformed records remain in the raw-record ledger
with a reason. They are not silently deleted from the source. The current
practice board supports square 9×9, 13×13 and 19×19; more formats can be normalized
later. Individual UTF-8 SGFs can also be uploaded through Game library, and their
originals are retained under the runtime data directory.

## Build a deployable release

Build on the same OS/architecture as the deployment target. For a Linux server,
clone the repository and build there:

```sh
git clone https://github.com/danielrich/contextual_go_training.git
cd contextual_go_training
./scripts/build-release.sh
```

The script produces `dist/whole-board-linux-x86_64.tar.gz` on x86-64 Linux, an
extracted release directory and a SHA-256 sidecar. It includes the binary, static
PWA assets, service template and installer. No corpus, database, private key,
credentials or development dependencies are included.

## Install the systemd service

Choose an existing non-root user that can read your KataGo executable, model,
source archive and trusted TLS certificate. The installer substitutes that user
into the unit; it does not change other services or start this one automatically.

```sh
sudo ./scripts/install-service.sh \
  --user "$USER" --release "$PWD/dist/whole-board-linux-x86_64"
sudoedit /etc/whole-board/whole-board.env
```

Set the engine/model paths and a certificate/key for the hostname used by your
phone. `WB_KATAGO_BIN` can point to an existing CUDA wrapper that forwards its
arguments. TLS files can be reused from another service at a separate port if
the selected user can read them. The service uses port **8443** by default, so it
can coexist with another app on 443.

Import into its persistent state directory as the selected service user:

```sh
sudo -u "$USER" /opt/whole-board/bin/whole-board import-json \
  --input /path/to/ogs_games_2013_to_2025-05.json.gz \
  --db /var/lib/whole-board/trainer.sqlite \
  --data-dir /var/lib/whole-board --limit 50000
sudo systemctl enable --now whole-board
./scripts/verify-deploy.sh https://your-go-server.local:8443
```

For a private CA, pass its public CA certificate as the verifier's second
argument. The phone must already trust the certificate issuer. Open the HTTPS
URL in Chrome and use **Install app / Add to Home screen**. HTTPS is required for
service workers when using a server hostname; localhost is allowed for development.

Configuration variables are documented in [deploy/whole-board.env.example](deploy/whole-board.env.example).
Service logs: `sudo journalctl -u whole-board -f`. Runtime engine logs are under
`WB_DATA_DIR/engine/`. GPU driver/CUDA dependencies are provided by the installed
KataGo executable or wrapper, independently of this Rust service.

## Practice away from the server

1. In the training room, choose board size and game speed.
2. Open **Offline practice**, choose the number of problems, and prepare a pack.
3. Preparation continues on the server. Complete problems are downloaded to this
   device; an incomplete problem is never counted as ready.
4. Check the **available offline** count before leaving. Use **Keep offline
   storage** to request persistent browser storage.
5. Open the installed app and practice. Use **Skip question** to advance without
   submitting or affecting accuracy, including skipping groups before candidate
   moves. Skips and unfinished notes survive reopening the app. Each submitted answer and reasoning note is saved
   locally before feedback appears. Unfinished sessions survive an app restart.
6. Reopen/focus the app when the server is reachable, or choose **Sync saved
   answers**. A retried attempt ID is accepted once, and conflicting content is
   rejected without overwriting the saved answer.

Ordinary online positions are quick to prepare and may have only partial
candidate analysis. An unsearched move remains **pending analysis**, rather than
being marked wrong. Downloaded ready packs have every legal candidate evaluated
and can complete all four questions without the server.

App-shell updates wait until you choose **Install app update**. Current sessions
are saved first. Browser storage can be cleared or evicted; the app reports what
is actually present rather than promising a cache that no longer exists.

## How grading works

- Winner: within 2 estimated points is “close”.
- Score: within **2 points or 20% of the signed lead**, whichever is larger, earns
  credit. A +30 estimate accepts +24 through +36. Exact error, average error by
  board/rank, and within-one/two-point counts remain visible separately.
- Candidates: a set earns credit if every alternative loses **less than 2 points**,
  or it includes the engine’s top move. Track losses **over 5** as blunders and
  **over 10** as serious blunders (also included in the blunder count). Pass is
  allowed. If fewer than three legal moves exist, ask for the available number.
- Game rank: mean both recorded ranks on a continuous kyu/dan ladder and round
  to the nearest rank; exact halves round toward the stronger rank. Unknown
  ranks remain Unknown. Both original ranks remain in the position details.
- First-time and review results are separate. Repeated positions do not improve
  main accuracy. **Review mistakes** reopens positions with incorrect answers or
  candidate losses of at least 2 points, with separate review point-loss metrics.
- Groups: secure-looking means ≥90% own-color ownership at every stone; at-risk
  means ≥70% opponent ownership at every stone; otherwise uncertain. These are
  ownership estimates, **not proofs of life, death or seki**.

Model identity, visit budget and grader version are stored with each problem.
New submissions use grader v3; already submitted v1/v2 results retain their
original scoring, and queued older answers remain sync-compatible.
Search noise can affect close calls. Synced answers are regraded by the server
from its saved reference rather than trusting a client-supplied correctness flag.
Neither practice accuracy nor the profile rank is an OGS rating prediction.

## Recount a position

Captured-stone totals and komi are visible before winner and score answers.
Capture totals come from replaying the recorded moves, including setup positions
and captures caused by rules permitting suicide. Older server references are
enriched from their retained games. An old offline bundle without capture history
says it is unavailable until reconnected; it does not assume zero prisoners.

After a wrong winner or score answer, choose **Recount on the board**. Click to
mark Black/White territory, mark dead connected chains, erase, undo or clear.
The worksheet shows territory, prisoners or living stones (depending on scoring),
komi and a partial total. It preserves your work locally across reloads and never
adds another graded answer. Exact engine-score comparison unlocks only after
you have committed a score answer.

**Show engine territory estimate** starts off. Turning it on permanently skips
group assessment for that session, even if the overlay is hidden later. Tinted
squares show ownership estimates at 60% confidence; orange borders flag confident
disagreements with your manual marks. Orange rings on stones indicate estimated
opponent ownership. The map is not final adjudicated territory and its squares
need not add up to the predicted final score. Unsettled play, seki, handicap and
rule-specific adjustments can differ from the manual worksheet.

## Update and back up

Build a new release, run the installer against it, then restart only `whole-board`.
The installer preserves `/etc/whole-board/whole-board.env` and the database.
Existing installed clients can finish cached sessions before installing the update.
Back up the SQLite database while the service/importer is stopped, along with
its runtime uploads if those originals matter to you. Problem references,
reference analyses and synced attempts all live in that database.

## Checks

```sh
cargo test --locked
npm ci --ignore-scripts
npm test
npm run check
python3 -m unittest discover -s tests
bash -n scripts/build-release.sh scripts/install-service.sh scripts/verify-deploy.sh
```

Rust tests cover setup/captures, SGF/JSON normalization, disk-backed selection,
checkpointed raw-record retention, authoritative grading, duplicate/conflicting
sync, cancellation and restart recovery. IndexedDB tests cover atomic pack writes,
reopening sessions, saved notes, failed sync and retry idempotency. The Python
files and tests are retained as the initial prototype/reference implementation;
use the Rust service with the current PWA frontend.
