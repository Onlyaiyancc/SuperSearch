# Search Experience Implementation Plan

> Execute in the current session with subagent-driven-development for the independent settings task and inline coordination for ranking/query changes. User approved implementation; do not pause between tasks.

**Goal:** Improve relevance, shortcut/alias entry and query efficiency in one verified local development revision.

**Architecture:** Preserve existing IPC, public result tiers and history policy. Extend ranking's internal sort key, isolate settings input helpers, and eliminate irrelevant catalog work before copying/matching.

**Tech Stack:** Rust, Tauri 2, Svelte 5, TypeScript, Vitest.

## Task 1: Ranking quality and bounded work

Files: `src-tauri/src/ranking.rs`, `src-tauri/src/ranking_corpus.rs`.

- [x] Add failing tests for `weixin`, `wx`, `wt`, history override, aliases, and partial retrieval.
- [x] Run `node scripts/desktop.mjs test ranking:: -- --nocapture --test-threads=1` and confirm relevance failures.
- [x] Introduce an internal match detail used after existing tier and before pin/usage tie breaks. Keep `match_tier` returning the original public tier.
- [x] Filter scope before normalization and postpone path-key creation until after matching.
- [x] Keep baseline corpus comparisons for candidate membership and unrelated file ranking; explicitly assert intended top-one changes.
- [x] Run focused tests; capture release component workloads before and after against the same fixtures.

## Task 2: Settings input experience

Files: `src/settings-input.ts` (new), `tests/frontend-settings.test.ts` (new), `src/App.svelte`, `src/styles.css`.

- [x] Add failing tests covering Windows shortcut serialization, modifier-only/IME/unsupported keys, Tab/Escape, full-width aliases, duplicate aliases and limits.
- [x] Run `npm test -- tests/frontend-settings.test.ts` and observe missing behavior.
- [x] Implement pure input helpers with no added packages. Use English helper/API output and Chinese UI labels/comments.
- [x] Wire the recorder and alias parser into existing forms, with inline errors and accessible descriptions; avoid letting recorder events reach global shortcuts.
- [x] Run focused tests and `npm run check`.

## Task 3: Query path efficiency

Files: `src-tauri/src/service.rs`, related tests in the existing module.

- [x] Establish repeated same-input release baseline for initial entries, including application and file scopes.
- [x] Verify the narrow catalog-loading guard by review; compare complete scoped results and repeated service workloads to preserve pins/history, app/all scopes and in-directory applications.
- [x] Implement the smallest scope guard around app catalog loading/ranking.
- [x] Run focused service tests and identical release workload; report timing as component evidence only.

## Task 4: Integration and review

- [x] Run `npm test`, `npm run check`, `npm run build`, `npm run desktop:test`, `npm run desktop:check`.
- [x] Build release desktop and run bounded bundled/native diagnostics if feasible; use only project desktop wrappers.
- [x] Inspect screenshots of shortcut feedback/alias dialog in light, dark and narrow layouts; verify keyboard behavior in browser preview.
- [x] Review specification compliance, then correctness/maintainability, and fix findings before rerunning affected checks.
- [x] Record fresh results and limits in `artifacts/archive/2026-10-19/outputs/Search-Experience-Improvement-Report-2026-10-15.md`; leave published baseline identity intact.
