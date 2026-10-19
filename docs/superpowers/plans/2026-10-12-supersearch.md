# SuperSearch Implementation Plan

**Goal:** Deliver a runnable Windows search launcher implementing the approved technical design in `docs/technical-design.md`.

**Architecture:** One Tauri 2 window and Rust process. A single Everything worker owns the official SDK, bounded queries and snapshots. SQLite persists apps, aliases, pins and successful launches; Svelte renders search and settings without a network service.

**Tech Stack:** Rust/MSVC, Tauri 2, Svelte 5, TypeScript, SQLite, official Everything SDK.

## Work units

Implementation and local test delivery completed on 2026-10-12. These checkmarks track implementation/local verification, not full public-release acceptance; the remaining native UI, lifecycle, coverage and performance gates are recorded in `artifacts/archive/2026-10-19/outputs/Acceptance-Report.md`.

- [x] Build environment: project-local Rust (no global PATH changes), locked dependencies, retained SDK license and SHA256; compiler version and frontend production build verified.
- [x] Domain (`query.rs`, `model.rs`, `ranking.rs`, `store.rs`): quoted phrases, reserved characters, scope conflicts, directory boundaries, Chinese/pinyin tiers, usage limits and settings tests executed.
- [x] Windows providers (`platform.rs`, `everything.rs`): Known Folder shortcuts and AppsFolder, native descriptors, SDK HWND/reply IDs, deadlines and candidate limits implemented. Missing-client and real existing-index queries verified; fresh fixture indexing remains unverified.
- [x] Desktop host (`main.rs`, `service.rs`): backend-owned results, stale request rejection, trusted Shell action paths, pins, native chooser, tray, shortcut recovery, single instance, opt-in autostart and hide cancellation implemented. OS-side action and IME validation remains pending.
- [x] Frontend (`src/`): four scopes, keyboard/IME guards, stable selection, source states, load more, actions and settings implemented. Frontend tests, type checks, production build and browser preview interaction checks passed.
- [x] Review: spec compliance and correctness review completed; all reported concrete P2 issues fixed and reviewed again.
- [x] Local test delivery: release executable/installer, README, sanitized native/startup evidence and acceptance report generated; exact delivered executable smoke checked successfully.

## IPC contract

Rust serializes camelCase. `start_search({requestId,rawQuery,scope})` returns initial `SearchSnapshot` and emits `search_snapshot` for file completion. `cancel_search({requestId})`, `load_more({requestId})`, `execute_action({requestId,resultId,action})`, `set_pin({requestId,resultId,enabled})`, `remove_pin({pinId})`, `get_settings()`, `update_settings({settingsVersion,patch})`, `refresh_apps()`, `get_provider_status()`, `start_everything()`, `choose_everything()`, `add_app({displayName,aliases})`, `set_app_aliases({appId,aliases})`, `clear_usage_history()`, `hide_window()` and `get_diagnostics()`.

`SearchSnapshot`: requestId, snapshotRevision, scope, complete, items, providers, truncated, hasMore. Item: resultId, entityId, kind (application/file/folder), title, subtitle, matchTier, pinned, available, actions. Provider: id, status, message. Settings: settingsVersion, shortcut, autostart, recordHistory, includeDesktop, hideHidden, hideSystem, includeDirectories, excludeDirectories, everythingPath, autoStartEverything, theme.

Results retain full locators exclusively in backend state. Backend command validation is authoritative. UI invalidates actions immediately on input/filter change and ignores older request/revision responses. Browser preview uses an explicitly labelled fixture adapter and never pretends to execute native actions.

## Continuation checkpoint — 2026-10-12

The existing code and local toolchain are preserved. Baseline verification: 10 frontend tests pass; 28 Rust tests pass; one opt-in live Everything probe is pending. The directory is not a Git repository, so no commit/worktree workflow applies.

### Task A: reproducible desktop launcher

**Files:** `scripts/desktop.mjs`, `package.json`, `tests/desktop-launcher.test.mjs`; retain the existing PowerShell script for compatibility.

- [x] Reproduced legacy PowerShell environment propagation failure; same script runs in current PowerShell host.
- [x] Wrote Node launcher behavior tests covering commands, child environment, PATH normalization and invalid modes.
- [x] Observed 10 intended failures before implementation, then 10 passing launcher tests.
- [x] Implemented standard-library Node launcher with isolated child environment and exit/error propagation.
- [x] Replaced desktop npm entries and added desktop:test/check/smoke.
- [x] Verified focused/full frontend tests and Rust suite through the Node entry.

### Task B: real native verification and delivery

- [x] Verified application discovery and missing-client state with real backend evidence.
- [x] Started the existing Everything client quietly; did not alter its index/service configuration.
- [x] Verified existing Chinese/name/folder/ext/directory/default-attribute queries; new project fixture remains explicitly unverified-not-indexed.
- [x] Fixed provider visibility, complete settings rollback, cancellation-before-acceptance and refresh setting races with red/green regressions and re-review.
- [x] Ran frontend checks/build, Rust checks, npm audit and release NSIS build; delivered artifacts in outputs. Rust audit and coverage remain unverified.
- [x] Added README, quickstart, acceptance report, hashes and startup/native evidence. Hidden process-tree memory sample exceeded the initial 150 MiB goal and is documented honestly.
