# Reliability, performance and search quality implementation plan

**Goal:** Execute the approved stability → startup/search performance → search quality → maintenance sequence with measured evidence.

**Architecture:** Preserve the local Everything worker and request-generation gate. Keep query submission atomic with generation checks, wait outside state locks, and merge only current results. Reuse existing isolated native diagnostics and installed tooling.

**Tech stack:** Rust, Everything SDK, Tauri 2, Svelte 5, Vitest 5, Node.js.

This folder has no Git metadata. Originals are backed up in `artifacts/archive/2026-10-19/work/optimization-2026-10-14/originals`; no Git commits or worktrees are available.

## 1. Stability
- [x] Capture baseline: 61 frontend tests; 86 Rust tests passed, one opt-in probe ignored.
- [x] Reproduce status-mailbox contention with a bounded receive test; switch status submission to nonblocking acquisition.
- [x] Add bounded, demand-driven recovery for an exited owned engine or stopped dedicated service. Retry at most twice per 30-second window; preserve manual restart and offline fallback.
- [x] Validate real recovery using only a diagnostic-owned child process in the bundled smoke fixture. Do not change installed machine services.

## 2. Startup and search performance
- [x] Record existing native UI benchmark before changes (failed: missing service/hidden window; not a comparable latency baseline).
- [x] Add a concurrency regression showing query submission holds the generation lock but reply waiting releases it; invalidate during the wait and reject stale merges.
- [x] Publish main file results before optional exact-name and folder queries; preserve the 250ms total budget and bounded candidates.
- [x] Precompute query normalization and ranking sort keys once per query/candidate. Preserve history, match-tier and pin priorities.
- [x] Extend the isolated bundled smoke diagnostic to record first-index readiness and cached-index restart readiness.
- [x] Run the native UI benchmark after rebuilding and report first-result and completion latency separately. Final benchmark uses 483 pre-created files, independent folder index and independent WebView2 profile; it is not comparable to the failed original whole-volume setup.

## 3. Search quality
- [x] Add failing regressions for Windows path duplicates differing by case or separators; preserve distinct application launch arguments.
- [x] Add fixed quality cases for Chinese, pinyin, initials, aliases, exact/prefix/substring, multi-keyword and directory-boundary searches.
- [x] Confirm existing history precedence and stable deterministic ordering.
- [x] Independent review found provisional default selection surviving a later exact match; failing regression added, automatic default follows latest ordering while explicit selection remains stable.

## 4. Engineering maintenance and verification
- [x] Constrain Vitest discovery to the project tests directory, covering frontend and desktop launcher `.test.mjs` tests. The original 61-test suite already included the launcher; no omission was confirmed.
- [x] Add a single verification command covering frontend tests/check/build, Rust tests/check and optional isolated smoke tests; stop on failures and save JSON results.
- [x] Add wrapper-supported Rust audit when available, retain production npm audit, and report unavailable audit tooling honestly (cargo-audit is not installed locally).
- [x] Correct current README/AGENTS dimensions, colors and test information. Keep historical acceptance reports unchanged.
- [x] Save fresh optimization evidence in `artifacts/archive/2026-10-19/outputs/Optimization-Report-2026-10-14.md` and delivery artifacts/reports in `artifacts/archive/2026-10-19/outputs/Optimization-2026-10-14/`.

## Acceptance boundaries
All added behavior must have regression evidence. Full machine installation/upgrade/UAC cancellation, real IME/multi-monitor interactions and long-duration resource soak remain explicit manual acceptance tasks; isolated smoke tests do not substitute for them. Performance figures are single-machine diagnostics, not cross-machine guarantees.
