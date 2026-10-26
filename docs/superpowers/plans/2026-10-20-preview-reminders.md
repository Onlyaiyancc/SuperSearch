# Preview and Reminders Implementation Plan

> **For agentic workers:** Use executing-plans for integration and dispatching-parallel-agents for independent native modules. Track the steps below.

**Goal:** Ship optional local file preview and persistent countdown reminders with compact extension settings.

**Architecture:** Each native capability owns its module and boundary validation. Existing search result tokens authorize preview reads. Reminder records live in SQLite and a single native scheduler handles due items independently of webview visibility. Separate Svelte panels reuse existing layout and settings.

**Tech Stack:** Tauri 2, Rust, SQLite, Svelte 5, TypeScript, Vitest; official native notification integration and safe Markdown rendering.

## File preview

- [x] Add native file fixture tests for bounded read, UTF-8/UTF-16, supported images, invalid format, directories and network/device paths; run via desktop wrapper and observe missing implementation.
- [x] Implement `src-tauri/src/file_preview.rs`: regular local files, text limit 1 MiB/images 10 MiB, byte signature validation and safe serialization.
- [x] Connect `preview_file` to `service.rs` current request/result lookup; release locks before reading and recheck opt-in/current result before returning.
- [x] Add frontend tests in `tests/frontend-extensions.test.ts` for explicit-selection shortcut routing and Markdown safe rendering. Run `npm test -- tests/frontend-extensions.test.ts` before implementation.
- [x] Implement `src/file-preview.ts`, `src/FilePreviewPanel.svelte`, settings, menu and keyboard routing in App; preserve search/query/selection, close on lifecycle changes.

## Countdown reminders

- [x] Add native tests in `src-tauri/src/reminders.rs` and `store.rs` for validation, capacity, persistence, cancel, disabled state and due-item claims; observe failures before implementing.
- [x] Add SQLite table without changing existing table semantics. Implement serialized create/list/cancel/due operations, bounded completed retention and one scheduler with cancellation wakeups. Register official notification plugin, notification status events and startup catch-up.
- [x] Add parser tests for independent command prefixes, compound units, bounds, title limits and ordinary filename boundaries. Implement `src/reminders.ts` and `src/ReminderPanel.svelte` with explicit submit, pending protection, list/cancel and visible-only countdown rendering.
- [x] Add default-off compatible settings in `model.rs`, `types.ts`, `bridge.ts`; saved disable cancels pending reminders, canceled draft does not. Show this effect beside disabled draft before save.

## Integration and verification

- [x] Read current Context7 documentation for introduced platform/library APIs; regenerate dependency notices for new graph.
- [x] Run `npm test`, `npm run check`, `npm run desktop:test`, `npm run desktop:check`, `npm run build`. Fix failures and inspect final diff.
- [x] Exercise production browser light/dark/narrow layouts, existing translation/search flows, safe preview, keyboard/IME, create/cancel/reload and stale replies. Save evidence in `test-results/preview-reminders/`.
- [x] Validate actual native file reading and scheduler/notification integration where feasible; clearly distinguish simulated UI from native notification delivery.
- [x] Build local executable/installer via `npm run desktop:build`, preserving any running executable by reversible rename if necessary. Update user docs and design QA. Do not publish or commit unrelated work.
