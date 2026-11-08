# Whole-board Go trainer

Approved scope: a browser app for a self-reported 10 kyu player aiming at 2–3 kyu over a year. Introduce the assessment sequence on 9×9, mix 13×13, emphasize 19×19. Track winner judgment, score estimation, group assessment, and three independent sound candidates separately. No promised rank gain or fabricated rank estimation.

Implementation choice: local Python HTTP server, SQLite persistence, dependency-free browser client, and installed KataGo analysis subprocess. Keep private archive and engine on this computer; no hosted deployment in this milestone. Practice must be usable without an archive via explicitly labeled introductory positions. Import SGF files/directories and reject correspondence or unknown-speed games from the verified live corpus. User supplied archive is read-only. Support legal replay including captures, setup stones, ko and passes. Preserve source, board size, rank metadata, komi and rules. Select from live games at all recorded ranks; track accuracy by original Black/White ranks. Include every playable phase through late endgame and the first pass; no move-number caps. Unknown ranks remain explicitly disclosed.

Question sequence: overall winner, signed score estimate including komi, group assessment (ownership-based estimate, explicitly not proof of life), and three unique legal candidates. Do not disclose evaluation before submission. Grade arbitrary candidates through forced engine analysis, not matching a top-three list. Use score loss from player's perspective with a stated two-point initial tolerance. Record attempts by skill, board size and source rank. Rank remains self-reported; performance is not OGS rating.

Feedback must remain unavailable if engine fails rather than fabricate scores. Show startup/analysis errors and allow retry. Progress is durable on this machine. Cache analysis, bound engine workload and timeouts, bind localhost only. No LLM-generated tactical claims.

## User revision, 2026-11-08
Remove all rank-based selection preferences. Track results for original game ranks separately. Remove endgame move cutoffs; counting in late positions is useful. Search the training server for a potentially larger archive beyond the initial 23,444-file subset.
