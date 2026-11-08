# Whole-board Go trainer Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement inline.

**Goal:** A working local training session with real KataGo grading and durable skill tracking.
**Architecture:** Python standard library service owns SGF replay, KataGo requests and SQLite. Browser client renders an interactive SVG board and a gated four-stage assessment.
**Tech Stack:** Python 3, SQLite, vanilla JavaScript/CSS, installed KataGo Metal engine.
**Spec:** docs/superpowers/specs/2026-11-07-go-trainer-design.md

## Global Constraints
- Never invent engine feedback or label sample games as historical OGS games.
- Keep input archive read-only; durable application data under data/.
- Explicitly disclose uncertain group status and self-reported rank.

## Review Focus
- SGF variations/escaped comments/setup and captures: test mainline replay and coordinates.
- Unknown-speed/correspondence records: test fail-closed eligibility.
- Arbitrary legal candidate outside root search: force its evaluation; test perspective conversion.
- Repeated submissions: idempotent attempt persistence.
- Failed engine startup: visible error, no made-up results, retryable.

### Task 1: Game ingestion and rules
Files: go_core.py, tests/test_core.py.
- [x] Write failing tests for mainline SGF parsing, captures, suicide, ko, passes and live eligibility.
- [x] Run python3 -m unittest discover -s tests, observe missing implementation.
- [x] Implement parse_sgf(text), replay(game, turn), play(board, color, move), eligible(game).
- [x] Run whole suite.

### Task 2: Engine and durable training API
Files: engine.py, store.py, server.py, tests/test_training.py.
- [x] Test player-perspective score loss, candidate validity, idempotent attempt storage.
- [x] Implement bounded persistent KataGo JSON process, SQLite persistence and HTTP training endpoints.
- [x] Run suite and actual engine smoke test on 9×9 and 19×19.

### Task 3: Browser training flow
Files: static/index.html, static/app.js, static/style.css, README.md.
- [x] Implement responsive board, four questions, score feedback, independent candidate marks, group selection and progress.
- [x] Add SGF import CLI, local launch instructions and readable status/error states.
- [x] Verify API flow end-to-end; browser check when supported.

## Verification record
22 unit/API tests; JavaScript syntax check; actual KataGo four-stage sessions on 9×9, 13×13, 19×19 and White-to-play forced searches. Independent review identified missing komi and unsupported suicide rules; regression tests failed first, then passed after fail-closed import fixes. No archive located; SGF adapters implemented, actual corpus ingestion awaits path.
