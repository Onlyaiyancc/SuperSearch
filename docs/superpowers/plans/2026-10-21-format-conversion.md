# Format conversion implementation plan

> **For agentic workers:** Use executing-plans for integration and dispatching-parallel-agents for independent, explicitly owned modules. Follow test-driven-development for behavior, then review integration before completion.

**Goal:** Deliver the approved local image/document conversion extension, including on-demand components, safe output, and PDF/OCR capabilities.

**Architecture:** Native Rust owns file/directory grants, bounded jobs, installation and execution. Svelte displays the draft, component state and native jobs. Independent adapters use native image processing, LibreOffice, Pandoc and a separately packaged PDF helper. The current uncommitted preview/reminder/translation work is the integration baseline and must remain intact.

**Tech Stack:** Svelte 5, Tauri 2, Windows APIs, Rust image, LibreOffice/Pandoc executables, packaged Python PDF helper.

## API contract

All JSON fields are camelCase. Source `ConversionSource` is `{id,name,extension,bytes}`; ids are native grants, never frontend paths. Output directory is `{id,label}` or null for source directory. `ConversionOptions` is `{target,quality,dpi,icoSize,pages,sheet,merge,ocr,csvDelimiter,csvEncoding}`. `ConversionJob` is `{id,status,target,items,completed,total,options}`, items are `{sourceId,name,status,outputs,error}` and outputs `{id,name,path}`. Status strings are queued/running/completed/failed/cancelled; item statuses pending/running/completed/failed/cancelled.

Commands: `choose_conversion_files`, `choose_conversion_directory`, `conversion_from_result(requestId,resultId)`, `release_conversion_sources`, `preview_conversion_csv`, `start_conversion(sourceIds,outputId,options)`, `list_conversions`, `cancel_conversion(id)`, `open_conversion_output(id,reveal)`, `get_conversion_components`, `install_conversion_component(id)`, `cancel_component_install(id)`, `remove_conversion_component(id)`, `import_conversion_component(id)`.

Events: `conversions_changed` and `conversion_components_changed` signal that the panel should refresh. Hidden panels do not poll. `ConversionComponent` is `{id,name,description,status,version,downloadBytes,installedBytes,source,license,error}`. Component ids: office, pandoc, pdf, ocr. Exact component manifests must be checked in before download is enabled; no fake URLs or checksums. Offline component archives can be imported through a native picker with the same verification rules. Project-produced components are built and delivered locally; publishing them is separate from implementing the extension.

### Task 1: Native image/job subsystem

Files: create `src-tauri/src/conversion.rs` and `src-tauri/src/conversion/`; tests colocated. Root integrator owns main.rs, model.rs, service.rs and Cargo.toml.

- [x] Add failing native tests for format compatibility, bounded source grants, no-overwrite output, real image roundtrip, cancellation and queue limits. Run with `node scripts/desktop.mjs test conversion::tests` and confirm missing behavior fails.
- [x] Implement `ConversionState::new(root: PathBuf)`, `grant_paths`, `choose_conversion_files`, and the API commands above. Store lightweight native file identities, reopen and pin only while staging, and recheck identity before reading. Translate preview errors at the conversion boundary.
- [x] Implement full-size image decoding with pixel/byte bounds, EXIF orientation and JPEG white compositing; require explicit page/frame choice for multi-frame inputs. Retain original source bytes. Real output must decode successfully before commit.
- [x] Route external work through `crate::conversion_components::convert(root,source,output_dir,options,cancel)` and obtain components through that module. Native job execution never holds store/cache locks while waiting.
- [x] Implement background serial queue, output grants, bounded retention, temp cleanup and termination-safe cancellation. Verify native tests pass before integration.

### Task 2: Native components and external adapters

Files: create `src-tauri/src/conversion_components.rs`, `src-tauri/src/conversion_process.rs`, `src-tauri/conversion-components.json`; tests colocated. Do not edit shared Cargo/main/service files; request dependencies from integrator.

- [x] Add failing tests for manifest validation, ZIP traversal/duplicate/link/size rejection, supported direction routing and cancelled/timeout subprocess handling.
- [x] Implement component status, pinned downloads, SHA-256 verification, staging/atomic activation, cancellation/removal and offline import. Reuse supported system LibreOffice; use normal installer UI for missing Office, never install system software silently.
- [x] Implement a process runner with fixed executable and argument vectors, hidden Windows processes, owned Job Object tree cleanup, 1.5 GiB memory cap, timeout and cancellation. Output is staged and validated by the job subsystem.
- [x] Implement Office and Pandoc routes with isolated profiles, CSV sheet/encoding handling, no macros, no remote resources and document output checks. PDF/OCR routes invoke the packaged helper using a fixed JSON request.
- [x] Verify with synthetic fixtures and real installed/downloaded engines; never mark an untested or missing package ready.

### Task 3: Conversion frontend

Files: create `src/conversion.ts`, `src/ConversionPanel.svelte`, `tests/frontend-conversion.test.ts`. Root integrator owns App.svelte, bridge.ts and types.ts.

- [x] Write failing tests: `expect(parseConversionQuery('转换')).toBe(true)`, `expect(parseConversionQuery('转换说明.docx')).toBe(false)`, `expect(commonTargets(['png','docx'])).toEqual(['pdf'])`; test stale refreshes and mixed batches.
- [x] Implement typed API contract, localized errors, target labels/quality-loss explanations and bounded options. Never expose an API accepting arbitrary source paths.
- [x] Build panel using existing utility-panel styles: disabled state, source picker/removal/reordering, compatible target selector, output directory, per-target options, component cards/install/import/cancel and job list/open/reveal/retry/cancel. Explicit user action is required to install or start.
- [x] Refresh from native change events, suppress stale updates after disposal, preserve draft through component installation, and expose `submit()` for the search bar Enter route. Use semantic labels and fit bounded/dark/narrow windows.
- [x] Run frontend conversion tests and `npm run check`; root supplies preview bridge cases and App integration.

### Task 4: PDF/OCR helper and packaging

Files: create `scripts/conversion-helper/` including locked dependency input, helper source, fixture tests, packaging script and documentation. Deliver archives under `artifacts/releases/Format-Conversion-2026-10-21/` and temporary fixtures under test-results.

- [x] Add failing tests against real generated PDFs/images for text DOCX/TXT, table XLSX/CSV, no-table errors, image PDF, PDF page export, picture-slide PPTX and OCR availability.
- [x] Implement single-request JSON stdin/stdout protocol; allow only fixed targets/options, bounded files/pages/pixels/output, no formula execution, and no arbitrary commands. PDF Word output contains editable paragraphs, PDF PPTX contains full-page images with accurate page geometry.
- [x] Implement OCR via fixed Tesseract path/chi_sim+eng data, generating editable text or raster-backed searchable PDF. Treat absent text/tables as explicit results/errors; never pretend scans have native text.
- [x] Build independent Windows helper archive containing its runtime, licenses, entrypoint and package manifest. Provide SHA-256/size metadata and local installation archive. No user Python/pip requirement.
- [x] Verify generated artifacts by independent readers and collect honest OCR availability/results. Make offline import usable even before any component release is published.

### Task 5: Integrate and verify

Files: `src/App.svelte`, `src/bridge.ts`, `src/types.ts`, `src-tauri/src/main.rs`, `src-tauri/src/model.rs`, `src-tauri/src/service.rs`, `src-tauri/Cargo.toml`, lockfiles and license/docs updates.

- [x] Add backward-compatible `conversionEnabled: false`, preserve other drafts, and expose native result resolution only through current search credentials.
- [x] Register managed native states/commands, result action, standalone command panel and Escape/Enter routing; never intercept ordinary filenames or trigger Bing for plugin commands.
- [x] Add preview-only synthetic component/source/job responses with explicit sample labels; no browser file writes, downloads or execution.
- [x] Protect application exit when tasks exist and cancel owned processes on shutdown. Hiding the window keeps jobs running.
- [x] Run `npm test`, `npm run check`, `npm run build`, `npm run desktop:test`, `npm run desktop:check`, helper tests and real component smoke; native release build is recorded below; inspect new diffs and third-party notices. Tests needing esbuild parent directory access may run with reviewed sandbox escalation.
- [x] Review spec compliance and code correctness, repair findings, then record exact completed directions, installation paths, real-engine checks and remaining limitations in docs/design/qa.md. Do not claim full delivery if component builds or second-batch directions remain unverified.

## Working state

Baseline frontend invocation encountered sandbox directory traversal denial in esbuild. Rerun the same baseline with reviewed escalation; this is an environment restriction, not evidence of a product test failure. Continue in the current feature checkout to preserve the existing uncommitted integration baseline; no destructive Git operations or automatic staging of other changes.

## Final local delivery (2026-10-22)

Implementation, review repairs and representative real-engine verification are complete. Frontend tests: 267 passed; native tests: 216 passed, three opt-in diagnostics ignored; source/frozen helper: 16 passed each; packaging: one passed. Native check is warning-free. Real native adapter smoke passed. The release executable/NSIS build succeeded using `CARGO_TARGET_DIR=.tools/conversion-release-target` because the existing running release locked the default output executable. The freshly copied executable passed isolated bundled-engine smoke; the running old application was preserved.

Local candidate and pinned PDF/OCR archives are in `artifacts/releases/Format-Conversion-2026-10-21/`, alongside Chinese instructions, notices, manifest/checksums and evidence. No commit, push, public component publication, system Office installation or Latest-Release update was performed. Narrow layout has DOM width evidence only, with no available new narrow screenshot; full installer lifecycle and public OCR redistribution remain outside this local verification.
