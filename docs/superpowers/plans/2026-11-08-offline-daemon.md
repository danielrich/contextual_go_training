# Rust daemon and offline PWA implementation plan

> Implement inline using the executing-plans workflow. The user approved the architecture and requested implementation and Git commits.

**Goal:** A deployable systemd-managed Rust service and installable PWA with complete offline problem packs and durable, idempotent progress sync.
**Architecture:** Rust/Axum serves the existing interface, SQLite corpus/progress and a persistent KataGo subprocess. A resumable background worker evaluates all legal moves for each offline problem; service worker + IndexedDB retain complete bundles, sessions and an attempt outbox on the phone.
**Tech stack:** Rust 2021, Axum 0.8, Tokio, rusqlite, flate2, serde, vanilla JavaScript, node:test.
**Spec:** docs/architecture/phone-offline.md (approved by user in this chat).

## Global constraints
- All recorded ranks are eligible; retain accuracy by original Black/White ranks and board size.
- No endgame move-number cutoffs. Include pass as a candidate.
- Preserve raw archive records. Import/index as a stream with bounded memory.
- No invented engine grading; an unevaluated candidate stays pending.
- Keep archive files, runtime data, credentials and TLS private keys out of Git.
- Prepare deployment artifacts and verify on the training server; installation must not disturb Chinese Reader.

## Review focus
- Partial/interrupted packs never appear fully available offline.
- Retry sync and restart must not duplicate or lose attempts or notes.
- Group ownership estimates never claim proven life/death.
- Cached legal moves include pass and respect engine ko rules.
- Large corpus selection/status queries do not load the entire collection into memory.

### Task 1 — Persistent Rust service foundation
Files: Cargo.toml, daemon/src/{lib,model,go,db,archive,main}.rs.
- [x] Commit baseline prototype, create codex/offline-daemon.
- [x] Write and run failing Rust tests for captures/setup, streamed JSON conversion, disk-backed selection and idempotent immutable attempts.
- [x] Implement normalized models, replay, bounded JSONL/gzip import and SQLite schema with indexed selection and import checkpoints.
- [x] Run cargo test --offline.

### Task 2 — Engine and pack API
Files: daemon/src/{engine,packs,http,grade}.rs; daemon/tests/api.rs.
- [x] Test candidate perspective, pack completeness, durable job restart and server-side grading of synced attempts.
- [x] Implement bounded KataGo queue, authoritative legal mask from policy, complete candidate maps, persistent pack jobs and HTTP endpoints.
- [x] Verify real engine preparation and all-move grading; verify HTTP API and interrupted job recovery.

### Task 3 — Offline phone app
Files: static/{app,offline,grader,sw}.js, manifest.webmanifest, icons; tests-js/.
- [x] Test deterministic grading and idempotent outbox behavior with node:test.
- [x] Implement atomic IndexedDB packs, persistent sessions/notes, retryable syncing, cached progress, install manifest/service worker and offline UI.
- [x] Verify cold offline launch, offline answer submission after restart, pack interruption and repeated sync through browser/API checks.

### Task 4 — Deployment and final verification
Files: deploy/whole-board.service, deploy/whole-board.env.example, scripts/{install-service,build-release,verify-deploy}.sh, README.md, .github/workflows/check.yml.
- [x] Build locked Rust release and package only source/static/deploy assets.
- [x] Verify build + run on the training server with isolated test data/port; do not change the Chinese Reader service.
- [x] Independent code review, fix important findings, run complete Rust/JS/Python verification.
- [x] Commit deployable implementation and report commit, install commands, test evidence and actual remaining limits.

## Verification evidence

- 18 Rust tests, 18 JavaScript tests and 30 Python reference tests passed; Rust formatting, JavaScript syntax, deployment shell syntax and diff whitespace checks passed.
- Independent review found raw snapshot deduplication, outbox starvation and mixed app-shell update issues. Each was reproduced by a failing regression test and fixed.
- Browser with the server stopped: cached launch, downloaded real-engine problem, draft/notes recovery after reload, winner/count/group grading. Reconnection: four answers synchronized, second sync left exactly one assessment per skill and zero pending answers. Candidate feedback included arbitrary alternatives and pass. Phone-width layout inspected at 390 pixels.
- A real complete 9×9 pack contains evaluations for all 63 legal moves. This validates completeness, not a 25-problem throughput target. Full 19×19 pack preparation has no latency guarantee yet.
- Linux x86-64 release built and served an archive position using installed CUDA KataGo. Bounded JSON import retained 1,100 raw records, normalized 1,096, and resumed from line 1,000. The full archive has not been indexed or independently counted.
- Production service installation and physical-phone HTTPS/app installation remain deployment steps; existing services were not modified.

## User feedback refinements

- Every unanswered question can be skipped without an attempt or accuracy penalty; skipped state and notes persist.
- Progress measures average candidate loss, sound-move percentage and move counts by board size and recorded Black/White rank pair. Pending feedback is excluded; sync retries do not duplicate metrics.
- Engine reference lines preview on the board via hover/focus or tap/click, with numbered alternating stones, capture/pass replay, stepping and original-position restoration. Replays never mutate the problem.
- A focused independent review caught an unpinned-preview restoration issue when leaving its controls; restoration handlers were added. Isolated browser verification covered step playback, focus restoration and pass numbering.
- Native Linux HTTPS passed health/PWA asset checks using an explicitly trusted temporary test certificate. Rendered systemd unit passed systemd-analyze verify.
