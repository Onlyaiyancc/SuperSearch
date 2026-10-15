# SuperSearch Agent Guide

Windows desktop search launcher (Tauri2 + Rust + Svelte5 + Everything SDK). Version 0.1.6, local test build.

## Command sequences

Frontend (Svelte/TypeScript):
```bash
npm ci                    # Install dependencies (package-lock.json locked)
npm test                  # Run Vitest frontend/tooling tests (139 tests)
npm run check             # svelte-check + TypeScript validation
npm run dev               # Vite dev server on http://127.0.0.1:1420
npm run build             # Frontend build only (outputs to dist/)
npm run verify -- --smoke  # Fail-fast project checks + isolated recovery smoke
```

Desktop (Tauri + Rust, requires local toolchain in `.tools/`):
```bash
npm run desktop:dev       # Tauri dev with hot-reload (launches Rust + frontend)
npm run desktop:test      # Cargo test (107 tests, 1 further opt-in ignored)
npm run desktop:check     # Cargo check (compilation validation)
npm run desktop:build     # Release executable + NSIS under src-tauri/target/release/
npm run desktop:benchmark # Native UI diagnostic (debug requires Vite dev server)
npm run desktop:audit     # Rust advisory audit; requires local cargo-audit
npm run desktop:audit:install # Install cargo-audit in the project-local toolchain
npm run desktop:soak      # 30-minute isolated native idle/search/window diagnostic
npm run release -- --with-diagnostics # Publish verified artifacts and paired evidence
```

**Important**: Desktop commands use `scripts/desktop.mjs` to set local toolchain paths (`CARGO_HOME`, `RUSTUP_HOME` in `.tools/`) without modifying global PATH. Never run `cargo` directly; always use `npm run desktop:*` wrappers.

Diagnostic modes:
```bash
# Bundled engine smoke test (isolated test directory, no shared Everything service)
SuperSearch.exe --smoke-test-bundled
node work/run-bundled-smoke.mjs

# Fast indexing smoke test (requires SuperSearch service running)
SuperSearch.exe --smoke-test-fast

# Legacy external client diagnostic
SuperSearch.exe --smoke-test
```

## Project structure

- **src/** - Svelte5 frontend (App.svelte entry, search-state.ts logic, bridge.ts for Tauri IPC)
- **src-tauri/** - Rust backend
  - **src/main.rs** - Entry point, tray menu, shortcuts, smoke test routing
  - **src/engine.rs** - Everything.exe integration and native indexing
  - **src/everything.rs** - SDK client wrapper
  - **src/service.rs** - Search orchestration, cache, window lifecycle
  - **src/store.rs** - SQLite persistence (settings, usage stats, pinned items)
  - **src/ranking.rs** - Scoring and prioritization
  - **src/native_smoke.rs** - Diagnostic test implementations
  - **resources/everything/** - Bundled unmodified signed Everything.exe + License.txt
  - **vendor/** - Everything SDK sources and original licenses
  - **windows/hooks.nsh** - NSIS installer hooks for service registration
- **tests/** - Frontend vitest tests (frontend-*.test.ts)
- **scripts/** - Build tooling (desktop.mjs launcher, icon generation)
- **work/** - Development scripts, finalization tools, QA artifacts
- **outputs/** - Release artifacts, documentation, UI screenshots, acceptance reports

## Architecture notes

- **Dual-layer search**: Everything SDK for NTFS fast indexing (MFT/USN journal) with fallback to directory scanning for non-NTFS volumes or missing service.
- **Separate service**: Machine-wide NSIS installer registers the dedicated service with SCM name `Everything (SuperSearch)` to avoid conflicts with personal Everything installations. User-mode client auto-uses its private pipe via IPC. Do not query the nonexistent alias `supersearch-everything-service`.
- **Chinese UI**: All user-facing text in Simplified Chinese (localization.ts), including tray menu, settings, error messages.
- **Window behavior**: Compact 680×76 default, no decorations, always-on-top, transparent. Expands to show results (172 + 76×rows, max 6 rows = 628px), settings 620px; bounded by monitor work area. Default hotkey Alt+Space; configurable via settings. Single-instance enforced (except `--ui-benchmark` mode).
- **Recovery and query concurrency**: Owned engine exit/service loss/restoration triggers bounded recovery on query/status demand (2 per 30s); forced diagnostic directory mode is preserved. No idle polling. Provider status IPC runs in a blocking task behind an async command. Check generation and submit under cache lock; wait for every IPC reply outside cache/store locks. Publish main file results before optional bounded supplements.
- **Application matching cache**: Shared FIFO cache holds at most 512 normalized titles and their pinyin/initial/acronym keys; titles over 512 UTF-8 bytes are excluded. Queries, launch paths, aliases and history are not cached. Aliases are evaluated live; title changes naturally change the content key. History-first ranking remains the product policy.
- **Launch modes**: Applications via pinyin/fuzzy match against registry/Start Menu, files/folders via Everything SDK. Bing web fallback when no local matches.

## Testing quirks

- **Frontend tests** run in Node/vitest without Tauri; `bridge.ts` functions are mocked.
- **Rust tests** include unit tests in `src/*.rs` `#[cfg(test)]` modules (107 tests plus one ignored). Run via `npm run desktop:test`, not raw `cargo test`. For test filters use `node scripts/desktop.mjs test ranking::tests -- --nocapture`. The 148-case relevance corpus and frozen baseline are test-only synthetic component evidence.
- **Smoke tests** are end-to-end diagnostics that launch the binary with special flags. They validate indexing, file discovery, and bundled Everything engine without touching user data.
- **UI benchmark** (`--ui-benchmark` flag) uses a temporary store and isolated fixture folder index, bypassing single-instance checks. Release UI/resource measurement: `powershell -NoProfile -File scripts/benchmark.ps1`; debug mode needs a Vite dev server. Timings distinguish first results and final completion.

## Build artifacts

Production build (`npm run desktop:build`) writes:
- `src-tauri/target/release/bundle/nsis/SuperSearch_0.1.6_x64-setup.exe` - NSIS installer (per-machine, installs service)
- `src-tauri/target/release/supersearch.exe` - Standalone portable executable
- This optimization publishes to `outputs/Priority-Optimization-2026-10-15/`, with matching manifest/checksums and `outputs/Latest-Release.json`. Publication refuses stale builds/checks or mismatched native/resource/raw-sample digests. `outputs/` also contains historical acceptance reports, screenshots and licenses.

`.tools/` contains project-local Rust/Cargo/npm caches; never commit this directory.

## Design constraints

- **Visual language**: User-selected silver/white minimal design (white #ffffff, text #181b20, charcoal accent #20242a, selected #eceff3), Segoe UI Variable typography, 25px search input, 18px result titles, 14px paths. See `design-qa.md` for visual comparisons and intentional native-size adaptations.
- **No cobalt/purple**: Keep primary surfaces neutral; semantic file-type colors remain available. Support light, dark and system themes.
- **Icons**: Use Lucide file-type icons and the generated black/white app mark. Derive PNG/ICO/tray assets from `public/supersearch-icon-source.png` through `scripts/build-icons.py`; avoid placeholder brand artwork.

## Known gaps

- Native full interaction testing (multi-screen, long-term performance, installation lifecycle) not fully automated.
- Short UI samples do not establish idle or long-duration use. Use `scripts/soak.ps1` for a separate bounded 30-minute protocol; its non-focusable synthetic windows do not establish physical interaction behavior. Measure memory growth rather than claiming absence of leaks from a pass status.
- One opt-in probe test currently ignored in test suite.

## Verification before commit

1. `npm test && npm run check` - Frontend validation
2. `npm run desktop:test && npm run desktop:check` - Rust validation
3. `node work/run-bundled-smoke.mjs` - Bundled engine diagnostic (if testing engine changes)
4. Visual QA against `design-qa.md` if touching UI

Use `npm run verify -- --smoke` for the automated sequence; report is saved to `test-results/verification.json`. Add `--audit-rust` when cargo-audit is installed. Do not treat historical reports as verification of a new build.

Avoid committing `.tools/`, `dist/`, `src-tauri/target/`, `node_modules/`, `test-results/`.
