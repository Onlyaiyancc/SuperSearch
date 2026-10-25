# Conversion Refinement Implementation Plan

> **For agentic workers:** Use executing-plans for task execution and dispatching-parallel-agents for the independent native/frontend ownership below. Follow test-driven-development and verification-before-completion. Keep unrelated uncommitted work intact; do not commit or push.

**Goal:** Clear incompatible merge state, preserve original retry intent and bound event-triggered status queries.

**Architecture:** Existing frontend option normalization and refresh helpers remain the single state boundary. Native retry selects only failed work using an existing job credential, then reuses queue validation and execution with the original output grant. Root integrates the fixed IPC contract and validates the combined feature.

**Tech Stack:** Existing Svelte 5/TypeScript, Tauri 2/Rust, Vitest and native colocated tests. No new dependency or adapter.

## Contract and ownership

- Frontend worker: `src/conversion.ts`, `src/ConversionPanel.svelte`, `tests/frontend-conversion.test.ts`. No bridge/main/native changes.
- Native worker: `src-tauri/src/conversion.rs`, including colocated tests. No main/bridge/frontend changes.
- Root: `src-tauri/src/main.rs`, `src/bridge.ts`, integration tests, docs, acceptance and local delivery.
- New command: `retry_conversion({jobId,sourceId}) -> ConversionJob`. `sourceId` is nullable: normal per-item retry supplies the failed item ID; merged retry supplies null to retry the original ordered batch. No destination/options/paths accepted from the frontend.
- Normal retry requires original job terminal failure and a failed item. Merged retry requires terminal failure and retains all sources/options. Native enablement, queue limits, file identity, output directory identity, no-overwrite commits and original CSV options still apply.

## Task 1: Source normalization and refresh helper

- [x] Add regressions, run `npm test -- tests/frontend-conversion.test.ts`, and observe the existing merge/concurrent-read failures.

```ts
const mixed = optionsForConversionSources(['png', 'docx'], {...defaultConversionOptions('pdf'), merge: true});
expect(mixed.merge).toBe(false);
expect(validateConversionOptions(['png', 'docx'], mixed)).toBe('');
```

- [x] Clear merge only when a nonempty batch cannot merge; preserve compatible image merge and empty draft state. Normalize restored drafts through the same helper.
- [x] Change `createConversionRefresh` to one shared in-flight pump with a dirty flag. Concurrent callers share the promise, mark one pending follow-up and never launch parallel reads. Ignore superseded/disposed publication. Allow later calls after failure.
- [x] Test burst coalescing with controlled promises: many calls start one read, resolution starts one follow-up, callers wait for the final read, only the latest eligible value is published. Cover errors and disposal without sleeps.
- [x] Run the targeted suite and `npm run check`; fix failures before reporting completion.

## Task 2: Native retry preserves original intent

- [x] Add failing colocated tests: original destination/options survive changing draft directory; merged source order remains unchanged; completed/running/queued/cancelled/unknown work cannot retry; invalid item IDs cannot select unrelated sources; queue capacity remains enforced.
- [x] Implement `ConversionState::retry(job_id, source_id)` and reuse common enqueue validation instead of duplicating format/limit checks. Clone only the selected original source grant(s), options and output grant while holding bounded locks; release locks before queue insertion/execution. Reject merged item retry in favor of null batch selection.
- [x] Expose `#[tauri::command] pub fn retry_conversion(app, job_id, source_id)` with the same enablement/wake/event lifecycle as `start_conversion`.
- [x] Use real tiny PNG fixtures to run a failed item's retry into the original directory; preserve existing output and append a collision suffix. Replaced source/output directory identity must fail rather than silently authorize a new path. Original reviewed CSV encoding/delimiter stay attached to its unchanged source.
- [x] Run `node scripts/desktop.mjs test conversion::tests -- --nocapture` using reviewed escalation when Windows fixture ACLs require it. Report exact results and any production warnings.

## Task 3: Panel and IPC integration

- [x] Frontend replaces `start_conversion` retry payloads with `{jobId:job.id,sourceId:item.sourceId}`; merged failures use a single `重试合并任务` action with sourceId null. Do not read current draft destination/options for retry. Preserve enabled/pending checks and localize expired task/invalid retry errors.
- [x] Subscribe job events to job refresh and component events to component refresh. Initial/manual refresh still reads both. No hidden-panel polling. Add a small testable event routing helper if needed to verify dispatch independently of the DOM.
- [x] Root registers `conversion::retry_conversion`, adds the browser preview rejection and extends `tests/frontend-conversion-integration.test.ts` to prove retry cannot cause native work in preview.
- [x] Review native/frontend contract, merged UI action and CSV/source identity behavior against the approved spec.

## Task 4: Verify and deliver

- [x] Run `npm test`, `npm run check`, `npm run build`, `npm run desktop:test`, `npm run desktop:check`; inspect actual command output and `git diff --check`.
- [x] Request a read-only correctness review of retry locking/credential boundaries, source normalization, refresh disposal and event routing. Repair verified findings, then rerun affected checks.
- [x] Build using `CARGO_TARGET_DIR=.tools/conversion-release-target` through `npm run desktop:build`, preserving the running old release. A locked target uses a different project-local directory, never force-closes user software.
- [x] Deliver to `artifacts/releases/Format-Conversion-Refinement-2026-10-22/`, keeping original conversion package unchanged. Copy component archives/metadata and notices, write concise Chinese instructions and exact binary/package checksums, include fresh verification evidence.
- [x] Execute the newly copied program's isolated `--smoke-test-bundled`, check its fresh JSON outcome, verify copied hashes match build output, and document exact test/build/UI coverage. If adapters are untouched, prior engine smoke remains explicitly scoped historical evidence rather than claiming a new adapter run.

## Baseline

Fresh baseline: 271 frontend tests passed across 20 files; Svelte check zero errors/warnings. Native tests 216 passed, three opt-in diagnostics ignored. Logs: `test-results/conversion-refinement-{baseline,check-baseline,native-baseline}.log`. Preserve ongoing preview-reading edits from outside this task.

## Completion evidence

All three approved changes passed spec and correctness review. Fresh full suite: 280 frontend tests; 222 native tests with three opt-in diagnostics ignored. Svelte check, frontend build and native check passed with zero warnings. The independent release build succeeded in 1m20s, and the copied new executable passed isolated bundled-engine smoke with a matching SHA-256.

Browser checks against the production frontend with synthetic native replies reproduced the old hidden merge and wrong-directory/single-image retry, then verified the repair. A synchronous burst of 30 job events dropped from 60 combined reads to two job reads and zero component reads. Component events refresh only components; disabling skips those reads while jobs remain visible. Dark/light 360px screenshots and equal 274px header client/scroll widths were inspected; browser warnings/errors were empty.

The optimized local candidate is in `artifacts/releases/Format-Conversion-Refinement-2026-10-22/`, with unchanged component archives, Chinese guide, license notices, checksums and paired evidence. No commit, push, public publication, Latest-Release update or system installation was performed. Adapter acceptance is explicitly historical for unchanged engines rather than a claimed new all-format run.
