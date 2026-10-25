# Translation and Dictionary Implementation Plan

**Goal:** Deliver the first optional built-in extension with explicit online translation and English dictionary lookup.

**Architecture:** Svelte panel isolated from file search; a small prefix/session module prevents late replies; Rust owns fixed-host HTTPS and clipboard operations. One persisted boolean enables the whole extension.

**Tech Stack:** Existing Svelte 5/Tauri 2; reuse locked reqwest 0.13.5 for HTTPS; serde_json for bounded provider responses.

- [x] Add frontend tests in `tests/frontend-translation.test.ts` for prefix boundaries, no implicit submission, stale success/error rejection and reset. Ran `npm test -- tests/frontend-translation.test.ts` and observed missing implementation failure, then implement `src/translation.ts`.
- [x] Add native validation/parser tests in `src-tauri/src/translation.rs`, register the module, observe failing compilation, then add fixed-host HTTPS requests, bounded response reading, a single-flight guard and lookup/copy commands. Register commands and extension state in `src-tauri/src/main.rs`.
- [x] Add `translationEnabled: false` to frontend/Rust settings defaults. Extend native settings tests to verify old configurations and persisted on/off values. Reuse store patch/version/rollback behavior.
- [x] Build `src/TranslationPanel.svelte` with explicit submit, direction selector, Chinese loading/errors, attributed results and copy controls. Add settings extension card, intercept prefixes and Enter in `src/App.svelte`, and reuse existing neutral design tokens in `src/styles.css`.
- [x] Add clearly marked synthetic preview replies in `src/bridge.ts`; no preview network access. Update both READMEs and `docs/design/qa.md` with behavior and fresh QA scope.
- [x] Run `npm test`, `npm run check`, `npm run build`, `npm run desktop:test`, `npm run desktop:check` and production browser QA. Probe services using only synthetic `hello` input. Review `git diff --check` and full changes before delivery. Do not commit or publish artifacts in this task.

## Acceptance examples

`parseTranslationQuery('fy hello')` returns `{ mode: 'translate', text: 'hello' }`; `parseTranslationQuery('fyi.txt')` returns null. A 501-byte translation fails before network access. Disabling the extension prevents the native lookup command. Resetting a pending frontend lookup prevents its eventual reply from becoming visible. `dict hello` displays English meanings and source attribution. Normal file search remains unchanged.

## Verification evidence

- Frontend: 240 tests passed; production build and Svelte/TypeScript validation passed with zero diagnostics.
- Native: 134 tests passed, one pre-existing opt-in probe ignored. Includes a local HTTP fixture for response limits and service errors.
- Browser: 18 production-frontend checks passed (nine preview, nine simulated-native); screenshots visually inspected in light 680px and dark 360px. No browser errors/warnings.
- Live synthetic `hello` probes: both public endpoints returned HTTP 200. These probes do not establish physical native interaction or long-term availability.
- Independent read-only review found no actionable issues. Local release executable and NSIS installer were built successfully. No commit, push or stable-release publication was requested.
