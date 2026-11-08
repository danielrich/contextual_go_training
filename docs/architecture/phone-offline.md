# Server daemon and offline phone practice

Status: implemented and validated in isolated local/Linux runs, 2026-11-08. Rust is the current backend; Python remains a reference. Production service installation and physical-phone installation are deployment steps. See README.md for commands.

## Deployment

Use the same pattern as the existing Chinese Reader: Rust/Axum serving the browser app and API, supervised by systemd, with HTTPS and an installable PWA manifest. Give Go its own origin (for example a separate HTTPS port) so its service worker and browser storage are independent of Chinese Reader. Inspect the host's actual deployment configuration before choosing the port or TLS arrangement.

Keep the archive, corpus index, KataGo process and authoritative synced attempts on the training server. Read the full JSON archive as a stream, preserving all raw records. Do not load the complete corpus into the current Store.games() interface. The Rust implementation uses a disk-backed SQLite corpus/cohort index. Record-specific limitations belong in metadata and problem eligibility; do not delete archival records.

## Practice packs

Start with an explicit Download 25 problems action and a visible available-offline count. A server worker prepares complete, versioned problem packs. Each problem includes:

- Stable problem ID, source game ID/URL, original Black/White ranks and rank provenance.
- Board size, rules, komi, player to move, position/move identity, board stones and group membership.
- Authoritative legal-move mask, including pass and ko restrictions.
- Winner and score reference, group ownership estimates, candidate evaluations and short reference continuations.
- Engine/model identity, search effort, grader version and grading tolerances.

Selection includes all recorded ranks and all playable game phases, including late endgame. Keep progress separated by source ranks, board size and question type. Preserve the existing question order, with feedback hidden until commitment.

## Offline grading

The expensive part is evaluating arbitrary candidate moves. Immediate offline grading requires the server to evaluate every legal candidate for that problem before marking it fully ready. A stored scalar score per legal move is compact; search preparation cost must be benchmarked on the training server's GPU. Never infer an unsearched move's score from policy probability or treat absence from the top three as a blunder.

If a candidate has no evaluation at the required search effort, save the answer as awaiting analysis and grade after reconnection. Do not count it as incorrect. An initially incomplete pack may support winner/count/group exercises, but the UI must identify which exercises are ready rather than claim complete offline readiness.

## Phone storage and sync

Cache the app shell with a service worker. Store packs, in-progress sessions, reasoning notes and an append-only attempt outbox in IndexedDB. Mark a problem ready only after an atomic successful bundle write. Request persistent browser storage where supported and show cache availability explicitly.

Generate attempt IDs on the device. Sync on app opening, focus and successful server reachability, with a manual retry action; background sync is an additional opportunity rather than the only mechanism. Server inserts must be idempotent by attempt ID. Keep submitted answers and grading versions immutable, and distinguish pending grading from completed grading. Update problems by version without invalidating work in progress. Multiple devices merge attempts instead of overwriting each other's counters.

## Validation milestone

One phone-installable PWA, one supervised daemon, and one 25-problem pack that supports the existing question flow in airplane mode. Verify a cold offline launch, interrupted pack download, offline app closure/reopening, queued attempts across restart, repeated sync, pass candidates, late endgame, and unchanged rank-separated results after reconnecting. Benchmark full candidate-map preparation before committing to a pack-generation latency target.
