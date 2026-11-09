# Capture counts and score-counting review

Implement the user's requested improvements on the current Rust daemon/PWA, then update the Linux service without changing Chinese Reader.

- Reconstruct actual captures alongside board replay, including setup stones, pass moves and rules permitting suicide. Preserve capture data for old saved problems by enriching them from their retained game; never guess zero for unavailable old offline metadata.
- Display both players' captured stones and komi before winner/score answers, without ownership spoilers.
- After a wrong winner or score answer, offer a counting workspace. Manual black/white territory marks and dead-chain selection update a transparent rule-aware subtotal, with undo/clear and preserved local work. Recounts are learning aids and never add graded attempts.
- Engine ownership/territory squares stay off until explicitly enabled. Once revealed, group assessment is permanently skipped for that session, including after hiding the overlay or reopening the app. Ownership is a probabilistic estimate, not a final adjudicated territory map.
- Keep the same starting board during winner/count review; do not autoplay engine continuations. Preserve original submitted feedback and accuracy.
- Verify captures, counting arithmetic, reveal/skip persistence, and old problem compatibility with focused tests, browser checks, and independent review. Commit/push/build/deploy the verified change and check the live service.

## Added score-tolerance requirement

Use ±max(2, abs(reference score) × 0.20). Winner judgment remains within 2 points
for “close.” New submissions use grader v3; older queued grading versions remain
supported. Track raw absolute counting error, average error by mean game rank and
board, and the fraction within one/two points separately for practice and review.

## Verification

- 28 Rust tests, 31 JavaScript tests and 30 Python reference tests passed.
- Independent review found an exact-score spoiler after winner feedback. Comparison
  is now disabled until score commitment; browser verification confirmed this.
- Cold-offline browser check: captures shown before either answer; no estimate
  squares until explicit reveal; manual territory/dead-chain arithmetic correct;
  marks and permanent group skip survive reload; continuation goes directly to
  candidates; recounts do not add progress entries; precise counting metrics shown.
