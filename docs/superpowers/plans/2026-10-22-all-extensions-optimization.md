# All Extensions Optimization Implementation Plan

**Goal:** Apply the approved four-extension interaction and resource improvements.

**Architecture:** Keep the existing Svelte panels and native contracts. Extract
the existing bounded refresh pump into a shared helper for conversion and
reminders; guard asynchronous feedback by panel/draft identity.

**Tech Stack:** Svelte 5, TypeScript, Vitest, existing Rust/Tauri commands.

Execute inline in the existing working tree to preserve the uncommitted feature
implementation. Do not commit or publish this mixed working tree.

## 1. Reproduce the interaction regressions

- [x] Add browser checks using the real app with synthetic native IPC: manual
  translation direction survives typing, reminder typing does not add reads or
  subscriptions, pending create replies do not mark edited drafts as submitted.
- [x] Add assertions for readable preview metadata and retained content during
  navigation, conversion batch-clear success/failure and pending-job retention.
- [x] Run against the unmodified app and record the expected failures.

## 2. Translation continuity

- [x] In `src/App.svelte`, key translation by mode and keep the reminder panel
  mounted during query edits. Pass launcher/document visibility to both panels.
- [x] In `src/TranslationPanel.svelte`, derive automatic direction from text and
  retain an optional manual override; reset lookup/copy presentation on query,
  direction, enablement or visibility changes. Reset override on hide.
- [x] Rerun the browser checks for explicit-only submission, stale replies,
  auto direction and manual override across edits and mode changes.

## 3. Reminder lifecycle

- [x] Move the existing `createConversionRefresh` implementation to
  `src/refresh.ts` as `createLatestRefresh`; preserve the conversion export alias.
- [x] In `src/ReminderPanel.svelte`, use the bounded pump for list reads, separate
  read errors from mutation errors, track draft generations, and keep mutation
  exclusion while an older draft's submission finishes.
- [x] Scope subscriptions to enabled/visible lifetime. Start a one-second visual
  timer only for unfinished reminders; clean up on hide/disable/unmount.
- [x] Rerun controlled-promise refresh tests and browser checks for burst reads,
  retry recovery, stale creation replies, disabled/idle timer and late cleanup.

## 4. Preview feedback and conversion clear

- [x] Test B/KiB/MiB formatting including zero, unknown and boundary values in
  `tests/frontend-extensions.test.ts`, observe failure, then implement in
  `src/file-preview.ts` and consume it in `src/FilePreviewPanel.svelte`.
- [x] Show type/size metadata and a reserved loading status using existing
  response data. Keep prior content during navigation and preserve image sizing.
- [x] Add `清空文件` to `src/ConversionPanel.svelte`; await one batch release
  before clearing sources, guard pending state and preserve destination/options.
- [x] Rerun browser checks, including release rejection and ongoing jobs.

## 5. Verification and delivery

- [x] Run `npm test`, `npm run check`, `npm run build`, `npm run desktop:test`,
  `npm run desktop:check` and inspect actual results.
- [x] Run browser regression checks with synthetic replies, capture and inspect
  light/dark/narrow screenshots. Record bounded coverage and any transient tests.
- [x] Inspect changes against the approved spec, update QA documentation and the
  plan, then report results without claiming a new executable release.
