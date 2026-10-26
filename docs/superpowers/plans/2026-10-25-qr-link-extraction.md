# QR link extraction implementation plan

User-approved extension of the current local usability-fixes candidate. Preserve existing fixes and uncommitted work; no commit or public publication.

- [x] Write failing native extraction tests; implement bounded local HTTP/HTTPS parsing, canonical deduplication, punctuation handling and native session-bound link actions.
- [x] Add explicit extraction, per-result link display/actions and stale-reply protection to the QR frontend, with preview-only synthetic replies and focused regression tests.
- [x] Verify production browser light/dark/narrow, empty/multiple/limit/failure/retry/cancel states; read-only correctness review and repairs.
- [x] Run full frontend/native tests/check, fresh release build and native smoke/centering diagnostics. Deliver a full portable ZIP retaining the three pinned component archives and all previous fixes.

## Verification

Completed locally on 2026-10-25. Native QR subset: 28 passed. Full suite: 258 passed, 3 existing probes ignored. Frontend: 345 passed. Svelte/TypeScript/native checks and release build passed. Final production browser suites 17+11 passed with matching asset hashes and zero errors/outbound requests. URL truncation findings (apostrophes, ASCII/Chinese separators and whole-URL punctuation) were reproduced before repairs and verified afterward. Independent re-review found no blockers.

Final executable SHA-256: bf4293089b99ea4553ecc9b22bd94b787ff4cc12262e1f2647448ab88db24a22. Its official Pandoc download/size/SHA probe passed in 3935ms, real-screen QR fixture in 243ms, bundled recovery in 660ms and native centering across 37 stable observations. The earlier real component/adapter proof applies to installer/converter sources confirmed unchanged. No dependencies were added by extraction; prior npm audit zero vulnerabilities, Rust seven allowed warnings and 395 verified notices apply to the unchanged dependency graph.

Full local portable delivery: artifacts/releases/Usability-And-QR-Links-2026-10-25/. All three component ZIPs retained and verified. No commit, publication or system Office installation. Physical multi-monitor/mixed DPI and physical shortcut presses remain unverified.
