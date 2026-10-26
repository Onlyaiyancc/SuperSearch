# Window, components and bilingual commands repair plan

User-authorized fixes on 2026-10-25. Preserve existing uncommitted extensions and QR work; no commit, publication or system-wide installation.

- [x] Window: reproduce top-anchored expansion in native geometry tests, then preserve visual center through compact/results/settings/preview resizing while fitting monitor work area and respecting manual dragging. Update obsolete anchor tests and validate small screens, negative positions and DPI.
- [x] Components: diagnose current Pandoc download/activation error, retain contextual messages, resolve exact pinned adjacent packages before online download, expose one-click installation and keep manual import as optional fallback. Verify packages against sizes/SHA and reject reparses.
- [x] Commands: add readable Chinese/English aliases for translation/dictionary/reminders/timer/QR, preserving old aliases and ordinary filename matching. Update visible examples and boundary tests.
- [x] Verify: real pinned Pandoc/PDF/OCR installation and representative conversion probes in isolated roots; frontend/native tests/check, browser states, release build, paired smoke evidence and a new local portable delivery including companion component archives.

Implementation ownership: root handles native window geometry, dependency diagnosis, package preparation, integration and delivery; component_install handles installer and component UI; bilingual_commands handles aliases/help.

## Verification so far

- Native geometry regression tests prove compact/results center retention, dragged centers, negative monitor origins, DPI scaling and work-area bounds.
- Real official Pandoc download through the app client passed in 3136 ms with exact size/SHA-256. Reqwest system-proxy support had been disabled by the existing default-features=false configuration; it is now explicitly enabled.
- All three local component ZIPs match the pinned trusted manifest. The opt-in real installation/conversion probe passed (115.12 s): native Pandoc/PDF/OCR install/probe, PDF/image/text/table/Office directions, Markdown/HTML/Word conversions, OCR and malicious document-resource rejection. These ran in an isolated root, not the user's installed app.
- Final frontend suite:21 files,337 passed. Native suite:247 passed,3 existing opt-in diagnostics ignored; the real component probe was separately opted in and passed. Native and Svelte/TypeScript checks passed.
- Production browser QA:17 synthetic groups passed, zero page errors/outbound requests, light/dark680/390px help, commands, component installation/error/import states inspected. Build hashes recorded in test-results/usability-fixes/browser-qa.json.
- npm audit:zero vulnerabilities. Rust audit exited successfully with7 allowed upstream warnings;395 Windows dependency notices verified.
- Read-only independent review found no concrete blockers in the four fixes.

Final native centering measurement passed:39 stable main-window observations,76px/324px heights, within2px of monitor work-area center. Startup14px tray-helper windows are excluded from the main-window collector. Final executable SHA-256:03ad89203de73e9bae470df68af3e426a4eb5652505e6ac1d222bb678f5aedf6. The delivered binary passed official Pandoc download+size/SHA checks in3665ms, screen QR fixture in249ms and isolated bundled recovery in592ms. Full local portable delivery includes all three exact pinned companion ZIPs; no public release or system-wide Office installation was performed. Physical multi-monitor/mixed DPI and physical keyboard presses remain unverified.
