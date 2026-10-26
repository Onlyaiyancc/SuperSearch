# Silver launcher visual QA

## Combined usability fixes and QR webpage links (2026-10-25)

The final combined candidate retains the centered layout, companion component
installation, Windows system-proxy support and bilingual commands. QR results
now offer explicit local extraction of up to 16 different HTTP/HTTPS addresses,
with per-link copy/open actions and original-content copying preserved.

Both production-browser suites were rerun against identical final asset hashes:
17 usability groups and 11 QR-link groups passed, with no page errors or external
requests. Light/dark/narrow screenshots were inspected. Native regression tests
cover Chinese punctuation, apostrophes, quoted values, canonical deduplication,
URL limits, forged IDs and preserving meaningful punctuation in a whole URL.

Final validation: 345 frontend tests, 258 native tests (3 existing opt-in probes
ignored), native/frontend checks and release build passed. Independent review
findings were reproduced and repaired, with no remaining blockers. The final
executable passed isolated native window centering (37 stable observations),
official Pandoc download/integrity verification (3935ms), native screen QR
capture (243ms) and bundled-engine recovery (660ms). All current smoke reports
are paired with its SHA-256. The installer/converter code remains unchanged
from the earlier successful real component/adapter probe in this session.

Local full delivery: `artifacts/releases/Usability-And-QR-Links-2026-10-25/`.
The portable ZIP retains all three pinned component archives. Physical mixed
DPI/multi-monitor, physical shortcut presses and system-wide Office installation
remain unverified; synthetic IPC browser checks do not establish those cases.

## Window, component installation and bilingual commands (2026-10-25)

The repaired native layout preserves the window's visual center while changing
height, including search results, extension panels and collapse. An isolated
native UI run measured 39 stable main-window observations at 76px and 324px;
all matched the monitor work-area center within two pixels. The collector
excludes the unrelated 14px startup tray-helper window. Unit coverage includes
six-row results, dragged centers, negative coordinates, DPI and work-area edges.

The complete portable package supplies pinned PDF, OCR and Pandoc archives in
`components/` beside the executable. Matching cards expose a single install
action without a picker or network request; manual import remains optional.
Published download paths use Windows system proxies and retain fixed HTTPS
hosts, size/SHA checks and cancellation. The delivered executable downloaded
and verified Pandoc in 3665ms. Real isolated installs and representative PDF,
OCR, Markdown/HTML and Office conversions passed a separately opted-in probe.

Canonical Chinese/English command pairs and prior shorthand are covered by
boundary tests. Seventeen production-browser groups passed with synthetic IPC,
zero browser errors and no outbound requests. Help, panel, installation and
error screenshots were inspected at 680px/390px in light and dark themes.

Fresh validation: 337 frontend tests, 247 native tests (3 existing diagnostics
ignored), frontend/native checks and release build passed. The component probe
was separately opted in and passed. The exact delivered executable passed QR
screen capture and bundled-engine recovery; dependency notices cover 395
packages. npm audit found zero vulnerabilities; Rust audit retains seven
allowed upstream warnings. Physical mixed-monitor/DPI, manual hotkeys and
system LibreOffice installation remain outside this local acceptance.

Evidence: `test-results/usability-fixes/`; local full portable delivery:
`artifacts/releases/Usability-Fixes-2026-10-25/`.

## Screen QR local candidate (2026-10-25)

The optional screen QR panel follows the existing silver/white extension layout,
with a QR-only thumbnail, escaped text, parsed domain, explicit copy/open buttons
and a neutral selected result. Final light/dark/narrow screenshots were inspected.
Fourteen production-browser scenarios passed using synthetic IPC replies, with
no page errors or outbound requests. They cover single/multiple/empty/error and
disabled states, keyboard actions, settings/shortcut error focus, late prepare
and scan replies, deliberate capture visibility changes, and passive/deferred
application-refresh regressions. Preview actions show an explanatory notice.

Fresh checks passed 304 frontend tests, 239 native tests (3 existing diagnostics
ignored), frontend/native checks, production frontend and standalone release
builds. The final executable passed the real-screen synthetic QR capture probe
at 2048 × 1152 in 238 ms and isolated bundled-engine recovery in 544 ms; reports
are paired with its SHA-256. npm audit found zero vulnerabilities. Rust audit
returned seven allowed upstream warnings; 395 dependency notices were verified.

Evidence and final screenshots are under `test-results/qr-scan/` and copied into
`artifacts/releases/Screen-QR-2026-10-25/`. Physical multi-monitor/mixed DPI,
hot-plug, physical global-shortcut presses and full native UI interaction remain
unverified. The screen probe saves no screenshot and does not prove those cases.

## Complete extensions local candidate (2026-10-24)

The latest four-extension source was rebuilt into a complete local executable
and NSIS installer. Integration review found that Escape from settings opened
over conversion cleared the underlying conversion state first. A production
browser regression failed before repair and passed after settings dismissal was
moved ahead of conversion navigation, preserving command text and selected-file
drafts. Modal/menu/preview precedence remains intact.

The Everything loading-to-ready fixture's polling worker could hold the mailbox
before status submission, causing the intentionally nonblocking status call to
return `loading`. The unchanged test reproduced this on stress attempt 11. The
fixture now waits for the real wake event before accessing the mailbox, then
releases it before disconnecting the reply. The corrected test passed 200 fresh
processes and the default parallel suite. Production status code is unchanged.

Fresh final checks passed 282 frontend tests, 222 default parallel native tests,
type validation, frontend/release builds, native check and bundled smoke. Three
native diagnostics remain ignored by default; the real component/adapter probe
was separately opted in and passed representative Office/Pandoc/PDF/OCR
directions with synthetic files. Production npm audit found zero vulnerabilities;
Rust audit exited successfully with six upstream warnings. Notices match 373
dependencies. Fifteen core browser cases, seven reading cases and seven adaptive
image cases passed with no browser errors; light/dark/narrow screenshots were
inspected. Browser IPC replies are synthetic.

The corrected copied executable, SHA-256
`fe9d016ff03d6facb3828f2e3123b1dd82a6852b32adcced77332ea6299437cf`,
passed isolated bundled recovery and short native UI diagnostics. Its 30-minute
protocol completed 60 cycles and 742 resource samples. Process-tree private
memory peaked at 236.14 MiB; first/last-quarter medians were 224.70/231.02 MiB
(+6.32 MiB), with a fitted slope of 35.47 MiB/hour. These observations do not prove
absence of leaks. Raw samples and paired report digests are retained. The first
candidate's intentionally interrupted run is separate from final acceptance.

Local delivery is under
`artifacts/releases/All-Extensions-Candidate-2026-10-24/`, with the portable ZIP,
installer, pinned local component ZIPs, 207-file source snapshot and 85 verified
delivery checksums. Internal version remains 0.1.7 and both binaries are
unsigned. No system installation or public publication was performed. Windows
10, physical IME/shortcuts, visible notification delivery, monitor hot-plug,
150%/200% scaling, sleep/wake and installation/cross-version upgrades remain
unverified. OCR public redistribution materials remain outstanding.

## All extensions optimization (2026-10-22)

Translation keeps a manually selected direction while the command text changes;
automatic direction still follows the text until a manual choice. Query edits,
mode changes, hiding and leaving the panel invalidate obsolete lookup/copy
feedback without submitting text automatically. Reminder typing keeps one panel
and subscription, with one shared in-flight list read and one pending follow-up.
Draft generations prevent a late creation reply from marking an edited command
as submitted; the mutation guard continues to prevent duplicate requests. List
read errors recover independently of create/cancel errors. Disabled, hidden,
empty and completed-only reminder panels have no visual countdown timer.

File preview shows content type, B/KiB/MiB source size and a reserved polite
loading status while retaining successful content during page/segment reads.
The information row adds a fixed amount of chrome: a 140×120 image remains at
natural size in a 391px shell with 17px bottom padding; a 1600×400 image fits at
600×150 in a 421px shell. Tall images remain capped at 620px and narrow windows
recalculate the fitted height. Older geometry thresholds were updated to allow
the added row; natural image dimensions, aspect ratio, padding and native height
dispatch checks remain strict.

Conversion exposes `清空文件`, releasing all selected credentials in one call
before clearing the selection. Rejection retains the retryable draft. The
operation remains pending across panel remounts and removes released IDs from
the latest saved draft while preserving newer selections, output destination,
conversion preferences and active jobs. CSV previews are invalidated on clear.

Fresh verification: 282 frontend tests passed; Svelte/TypeScript validation and
production frontend build passed with no errors or warnings; native check
passed. The standard parallel native suite encountered the same pre-existing
Everything loading-to-ready timing failure seen before these changes (221
passed, one failed, three ignored). The complete serial suite, run through
`node scripts/desktop.mjs test -- --test-threads=1`, passed 222 with three opt-in
diagnostics ignored. No Rust source was changed in this optimization.

Fourteen core production-browser scenarios passed, covering direction retention,
explicit submission, stale replies, reminder typing/subscriptions, burst reads,
mutation exclusion, independent errors, disabled/completed/hidden timers, delayed
list responses, metadata/navigation retry, batch clear success/failure, remounts,
CSV invalidation and light/dark/narrow layouts. Seven existing reading scenarios
and the seven-check adaptive image protocol also passed. Screenshots were
visually inspected, and browser errors were empty. All browser IPC replies and
data were synthetic: these checks did not send translation text, generate real
notifications, download components or convert user files. Evidence and scripts
are under `test-results/all-extensions/`, including `production.json`,
`preview-reading/ui-qa.json` and `adaptive-preview/qa.json`.

Independent read-only review against the pre-edit snapshots found no actionable
issues in the scoped changes. Review/check dispatch was initially interrupted
by approval-service usage limits; after the user requested continuation, those
checks and the independent review completed. This work updates source and the
frontend build only; it does not publish or package a new desktop executable.

## Conversion workflow refinement (2026-10-22)

Three approved refinements address incompatible hidden merge state, original-task retry semantics and redundant status reads. The native `retry_conversion` command accepts only job/source credentials, retains the original source identities, destination, CSV settings and options, and reuses the normal queue validation. Merged failures expose one batch retry instead of individual image retries. Source and destination replacement still fail during execution; output collisions preserve existing files.

Fresh full verification passed 280 frontend tests and 222 native tests (three opt-in diagnostics ignored). Svelte/TypeScript, frontend production build and native check passed without warnings. Six new native tests cover actual PNG retry/output decoding, original directory and collision handling, merged order/options, invalid states/foreign IDs, queue limits, replaced source/output identities and CSV source/options retention. The frontend conversion suite has 24 tests, including controlled-promise burst/error/disposal/completion-boundary checks. Independent spec and correctness reviews found no remaining scoped issues.

The production frontend was tested with explicitly synthetic native replies using the browser UI. Before repair, an image merge plus DOCX left an invisible merge flag blocking submission; the repaired form enables submission. The old merged retry submitted one image with `merge:false` into the changed draft directory. The repaired single and merged actions send only `{jobId,sourceId}`, with null source ID for the merged batch. A synchronous burst of 30 task events previously caused 30 task and 30 component reads; it now causes two serialized task reads and zero component reads. A component burst causes two component reads only. With the extension disabled, component events cause no reads and task events still refresh jobs. These counts describe the browser fixture, not a measured native speedup or document conversion time.

Light and dark 360px screenshots were inspected; both job headers measure 274px width and 274px scroll width, so the merged retry action fits without horizontal overflow. Browser warnings/errors were empty. Evidence is under `test-results/conversion-refinement-*`, including before/after request logs, DOM snapshots, UI JSON and screenshots. UI fixtures exercise IPC payloads and lifecycle rather than actual Windows dialogs or engine execution. Conversion adapters, component versions, package hashes and dependencies are unchanged, so initial real-engine acceptance below remains scoped to its original run.

The optimized Windows executable and NSIS installer built successfully via the project wrapper in `.tools/conversion-release-target`. The copied new executable passed a fresh isolated bundled-engine diagnostic, and its SHA-256 matches the build output. Delivery is separate at `artifacts/releases/Format-Conversion-Refinement-2026-10-22/`, with unchanged pinned PDF/OCR packages, checksums and paired evidence. No public release pointer or system installation was changed.

## Preview reading controls (2026-10-22)

PDF supports strict integer page jumps and preserves the displayed page after a read error while retaining the requested retry target. Images/PDF offer fit, 100% and bounded 10%–400% zoom; manual zoom uses a scrollable raster without additional native reads. PDF page changes retain zoom. Focused content supports PageUp/PageDown for PDF/text, left/right PDF navigation in fit mode and Ctrl +/-/0 zoom controls. Inputs, focused buttons, IME, repeated events and unrelated modifiers retain their native behavior.

Fresh validation: 271 frontend tests passed across 20 files; Svelte/TypeScript and the production frontend build passed with zero diagnostics; desktop check passed. Seven production-browser reading scenarios covered invalid/no-request page jumps, Enter submission, keyboard focus isolation, raster overflow and persistent zoom, failed-page indicator/retry consistency, small-image expansion/restoration, text segment navigation and light/dark/360px layouts. Seven adaptive-image regression checks also passed: 140×120 content uses a 367px window including the compact zoom toolbar, retains its original size and leaves 17px bottom padding. Screenshots in `test-results/preview-reading/` were inspected. Browser errors were empty. UI fixtures use synthetic native replies and do not establish physical keyboard, multi-monitor or installer lifecycle behavior.

## Local format conversion (2026-10-22)

The default-off conversion extension provides the exact commands `转换` and `convert`, a file-result menu action, bounded drafts and serial native jobs. Static image conversion is built in; Office, Pandoc, PDF and Chinese/English OCR components are available on demand. Original files remain intact, collisions receive new names, and hiding the panel leaves native jobs running. CSV inputs require an explicit bounded preview with selected encoding and delimiter.

Fresh frontend validation passed 267 tests across 19 files; Svelte/TypeScript and the production frontend build passed with zero diagnostics. The final native suite passed 216 tests with three opt-in diagnostic tests ignored. Source and frozen PDF-helper suites each passed 16 tests, including actual English and Chinese OCR; the license packaging test passed. Production npm audit reported zero vulnerabilities. Rust audit exited successfully with the five existing allowed upstream warnings; generated notices now cover 373 dependencies.

The latest real native adapter smoke passed in 154.69 seconds. It verified pinned ZIP import/probes and damaged-helper repair; PDF to PNG/JPG/TXT/DOCX/XLSX/CSV/picture-slide PPTX; no-table errors; merged-image PDF; OCR to TXT/DOCX/searchable PDF; Pandoc Markdown/HTML/TXT/DOCX including images; isolated LibreOffice DOCX/XLSX/PPTX to PDF, selected-sheet CSV, image-bearing HTML, and GB18030 semicolon CSV to XLSX with formula-like values preserved as literal text. These are real engine executions using synthetic files, with no system Office installation. They establish representative directions rather than every legacy format permutation or layout fidelity.

Review regressions cover HTML sanitization/CSP, refusal of executable media extracted from a crafted DOCX, ZIP paths and expansion bounds, exact Windows inherited-handle lists, descendant cancellation/cleanup, lightweight file identities, no-overwrite commits and stale UI responses. The Office HTML collector excludes its private profile before scanning image sidecars; published sidecars require a known raster extension, matching detected format and a bounded successful decode.

The production-browser conversion form was exercised for opt-in, default PNG, duplicate selection, remove/re-add draft retention and keyboard settings navigation. A light screenshot is saved at `test-results/conversion-ui-light.png`. A 360px DOM check measured page width 360px and panel width 358.4px; narrow screenshots were unavailable in the browser tool, so this does not establish a complete visual narrow/dark acceptance. Browser preview uses clearly labeled synthetic files and cannot convert or install components. Physical native keyboard, multi-monitor and installer lifecycle remain outside this run.

Evidence: `test-results/conversion-{frontend-final,frontend-build-final,rust-full-final,rust-check-final,helper-final,helper-frozen-final,package-final,adapter-smoke}.log` and `test-results/components-verification/native-adapter-report.json`. Local PDF/OCR archives live under `artifacts/releases/Format-Conversion-2026-10-21/`. These are local verification packages, not a public component publication; repackaged OCR redistribution still requires the dependency materials listed in its provenance notices.

The production Windows executable and NSIS installer built successfully in a separate project-local target directory because the user's running old release held the normal output executable open. The copied new executable's SHA-256 matches the build output. Its fresh isolated bundled-engine diagnostic passed all checks (`test-results/bundled-native-smoke.json`), without closing the user's running version or installing system components. Local delivery includes matching checksums, license notices and verification evidence; the published Latest-Release pointer is unchanged.

## Expanded file preview (2026-10-21)

PDF previews use the native Windows renderer, one cached document and one current PNG page (100 MiB input, bounded raster output). DOCX/XLSX/PPTX use bounded ZIP/XML extraction for content rather than layout fidelity (50 MiB input). Images allow 50 MiB and 64 million decoded pixels, including TIFF/ICO through WIC; larger rasters become thumbnails. Text/code/configuration files allow 1 GiB with approximately 1 MiB per segment and UTF-8/UTF-16/GB18030 boundary handling. HTML/SVG remain source text. Office truncation, worksheet/slide selection and decode/retry errors retain the compact neutral interface.

Fresh validation: 250 frontend tests passed; Svelte/TypeScript check and production frontend build passed without diagnostics. Rust tests passed 179 with two explicitly ignored live probes; desktop check passed without warnings. Native fixtures cover real PDF page rendering/cache identity/cancellation, DOCX/XLSX/PPTX extraction and malformed archives/XML, >10 MiB BMP thumbnails, TIFF/ICO, pixel limits and multibyte text boundaries. Non-PDF reads use a nonblocking single-work gate; closing a PDF cancels stale ownership and releases cached resources without waiting on rendering.

Dependency audit upgraded the new parsers to quick-xml 0.41.0 and zip 9.0.5, resolving five reported vulnerabilities. The final Rust audit exits successfully with five allowed upstream warnings (proc-macro-error, zbus_names, zvariant, zvariant_derive and glib); production npm audit reports zero vulnerabilities. Generated notices cover 342 dependencies.

Ten production-browser scenarios with simulated native replies passed for PDF bounds/page retry, token-only dispatch and stable session IDs, exact text offsets, backwards navigation and retry history, worksheet/slide switching, escaped Office/HTML content without network requests, repeated damaged-image retry, close during page rendering and native hide during initial load. Seven adaptive-image checks still pass: a 140×120 image uses a 329px shell, wide images 359px, tall images cap at 620px, with about 17px bottom padding. Screenshots in `test-results/expanded-preview/` (PDF, DOCX, PPTX, Office light/dark/narrow and text) were visually inspected. Browser errors were empty. These UI fixtures do not establish physical native keyboard, multi-monitor or installer behavior.

## Reminder duration units (2026-10-20)

Reminder durations accept case-insensitive s/m/h/d and second/minute/hour/day aliases alongside Chinese units, including compact, spaced and mixed-unit combinations. Parser checks cover bounds, unsupported suffixes and preserving a time-like phrase in the reminder title. All 248 frontend tests passed. Seven production-browser scenarios with simulated native replies passed, including six English-duration submissions that verified exact seconds and preserved titles. No native scheduler or storage contract changed.

## Reminder notification identity (2026-10-20)

The previous Tauri notification adapter deliberately omitted the application ID when the executable lived in `target/debug` or `target/release`, which made Windows attribute reminders to PowerShell. Reminders now register the application's current-user identity and embedded app icon, then use the Windows WinRT notifier with `com.supersearch.desktop` explicitly. The system header owns the SuperSearch name/icon; the content contains only the reminder text.

Two native tests verify configuration identity and a single safely escaped text node. An explicitly invoked live probe was accepted by Windows; registry readback confirmed `DisplayName=SuperSearch`, and the registered icon's SHA-256 matched the bundled PNG. This verifies the actual native send path and identity registration, not a separately captured physical toast screenshot. Normal tests leave this live probe ignored to avoid unsolicited notifications.

Implementation references: [Microsoft unpackaged notification identity registration](https://github.com/microsoft/WindowsAppSDK/blob/main/dev/AppNotifications/AppNotificationUtility.cpp) and the locally installed Tauri notification adapter's `desktop.rs` installed-path check. No dependency-source patch is used.

## Adaptive image preview height (2026-10-20)

Image previews now request a separate bounded native window height, calculated from the image's natural aspect ratio, available content width and measured controls/padding. Small images are not enlarged; large images preserve their ratio within a 620px logical-height limit and monitor work-area bounds. Text and Markdown keep their scrollable layout.

Production-browser measurements at the native 680px width: a 140×120 image uses a 329px window, with 17px padding below the image; a 1600×400 image uses 359px; a 200×2000 image caps at 620px. Width changes recompute height, and Esc restores the original query/results. Seven sizing/restoration checks and six simulated-native lifecycle scenarios passed without browser errors. Light small/wide/tall and narrow screenshots in `test-results/adaptive-preview/` were inspected. Native geometry tests cover height limits, DPI scaling and a constrained work area; physical multi-monitor interaction remains outside this browser evidence.

## File preview and countdown extensions (2026-10-20)

The extension page now has three default-off cards, each retaining the compact header-only disclosure design. Twelve production-browser scenarios passed for opt-in/save/cancel, normal query spaces, explicit selection + Space, menu preview, Esc query preservation, unsupported format, IME, reminder creation/list/cancel and saved-disable cancellation. Light and dark/narrow screenshots under `test-results/preview-reminders/` were visually inspected; no horizontal overflow or browser errors were observed.

Six additional production-browser scenarios with simulated native replies verified token-only preview dispatch, sanitized Markdown DOM with no external resource request, late reply disposal on close/hide, file read failure and explicit open, duplicate pending reminder submission, and panel lifecycle after hide/show. These are frontend IPC lifecycle fixtures, not physical Windows notification evidence.

Native tests use actual local text/image fixtures and a directory junction to check file type, encoding, byte limits, dimension limits and local-file enforcement. Reminder tests verify additive schema, input/capacity limits, persistence, cancellation, at-most-once due claims, retention, disabled state and wake-time calculation. Full native tests passed with the pre-existing opt-in Everything probe ignored. Real toast visibility, notification settings, sleep/wake timing and installed/portable notification identity have not been independently exercised. Image decoding is delegated to WebView; pixel bounds limit canvas dimensions, not aggregate animation work.

## Translation and dictionary extension (2026-10-20)

The extension card now defaults to a compact 78px layout containing only its icon, title, subtitle and enable switch, without a separate disclosure label or arrow. Clicking its title area uses native disclosure behavior to reveal examples, provider/privacy text and limits (339px in the same browser viewport). Ten production-browser checks confirmed default collapse, independent switching, click/Enter/Space disclosure, collapsed reentry with draft preservation, saved enablement and dark 360px layouts without horizontal overflow. Light and dark/narrow screenshots in `test-results/extension-header/` were visually inspected; browser errors/warnings were empty.

The production frontend passed 18 browser checks: nine clearly marked preview scenarios and nine scenarios with simulated native command replies. They cover default-off entry, extension save/cancel and preservation of unrelated drafts, English/Chinese direction selection, dictionary presentation, explicit submission/copy, UTF-8 length limits, no implicit local/online lookup, duplicate submission, quota/retry, IME protection, and stale replies after query edits, direction changes, native hide/show and settings navigation. Ordinary file search still returned its four design fixtures. Browser errors/warnings were empty.

Local screenshots under `test-results/translation-extension/` were inspected in light 680px and dark 360px layouts. The existing neutral surfaces, typography, fixed footer, scrolling settings and readable copy/error controls are retained, without horizontal overflow. These screenshots use synthetic replies; they are not native service or clipboard evidence. No physical Windows interaction, installation lifecycle or long-duration test is established by this browser run.

Separate real HTTPS probes using only the synthetic word `hello` returned HTTP 200 from both MyMemory and Free Dictionary API. Native tests exercise validation, byte limits, bounded response reads, HTTP error mapping, parsing, opt-in persistence and single-flight release with local fixtures. Public-service reachability is a point-in-time observation, not an availability guarantee.

## Loading status refresh (2026-10-19)

The production frontend passed 12 browser checks with simulated native status replies: loading-to-ready without reopening settings; no probes in general settings or after readiness; serialized slow probes; cancellation on tab exit, search return, native hide and document invisibility; resumed checking on visibility; stale reply rejection. Loading and ready screenshots in `artifacts/releases/Status-Refresh-Fix-2026-10-19/` were inspected. The existing status card and Chinese messages remain visually intact, with no browser errors or warnings. This is frontend lifecycle evidence, not a measurement of physical indexing speed.

## Alias editing regression (2026-10-18)

The production frontend passed 15 browser checks with simulated native command replies: existing alias prefill, cancel preservation, normalized edit/save/reopen, unchanged legacy comma-bearing labels, failed-save draft retention, explicit clearing, validation focus, and an empty new-application draft. Screenshots in `artifacts/archive/2026-10-19/output/playwright/optimization-2026-10-18/` were visually inspected in light 680px and dark 360px views. Neutral surfaces, readable help/error text, visible save/cancel controls and no horizontal overflow were retained. Browser warnings/errors were empty. These checks do not exercise native application launching or OS shortcut registration.

## Search experience settings improvement (2026-10-15)

The production frontend passed 23 browser interaction checks, including modifier/key recording, shifted digits, numpad identity, Latin keyboard layout labels, Tab/Escape, invalid-save focus, mocked native registration failures, Chinese-comma alias submission and normalized deduplication. Five fresh screenshots are in `artifacts/archive/2026-10-19/outputs/Search-Experience-2026-10-15/Preview-QA/`; light settings and dark 360px shortcut/alias error views were visually inspected. Neutral surfaces, labels, focus rings, readable inline errors, scrollable settings and accessible footer actions are retained; the narrow viewport has no horizontal overflow. Browser warnings/errors were empty. The command-reply harness does not register OS shortcuts or establish physical keyboard/IME behavior. Full fresh checks and measurement limits are recorded in `artifacts/archive/2026-10-19/outputs/Search-Experience-Improvement-Report-2026-10-15.md`.

## Priority optimization regression (2026-10-15)

Ordinary preview remains interactive after adding diagnostic-only input isolation. A headless Chrome run against the production frontend verified four design sample results, ArrowDown selection, settings opening, dark theme save and a 360px viewport without horizontal overflow. Console warnings/errors were empty. Screenshots `artifacts/archive/2026-10-19/outputs/Priority-UI-QA-2026-10-15/{compact,search,settings,dark-narrow}.png` were visually inspected against the established silver/white layout. The app and settings hierarchy, neutral surfaces and file-type icons are retained. The screenshot paths contain preview fixtures, not the user's file results.

The long native soak is explicitly non-focusable and inert to manual UI input; it exercises synthetic queries and native visibility/geometry. Short native UI diagnostics retain foreground focus. Browser preview does not establish native dragging, physical IME, multi-monitor, installation or sleep/wake behavior. Fresh build and native performance evidence are in the priority optimization report; historical sections below retain their original build scope.

Date: 2026-10-14. Selected direction: concept 1, silver/white minimal.

## Reference and evidence

Source: artifacts/archive/2026-10-19/outputs/Silver-UI-QA/reference.png (1487 × 1058), the user-selected generated concept. The previous paper/green design is superseded.

Actual browser screenshots: search-final.png and settings.png (900 × 820 viewport, 680 CSS px app width) in artifacts/archive/2026-10-19/outputs/Silver-UI-QA. search-comparison.png and settings-comparison.png place cropped reference and actual implementation together. Reference panels are normalized to 680px wide without changing their aspect ratio; actual panels are 680 × 476 and 680 × 620. Both comparison sheets were visually inspected together with the application icon size sheet.

Additional evidence: compact-final.png, narrow-search.png, narrow-settings.png, dark-search.png, menu.png, add-dialog.png and no-results.png. Preview paths explicitly contain 预览样例; they do not represent the user's files.

## Required surfaces

- Typography: Segoe UI Variable / Microsoft YaHei UI, 25px search, 18px result titles, 14px paths. Clear title/path hierarchy, truncation and keyboard hints inspected.
- Layout: 680px native width; 76px compact; 74px inner search, 46px scope strip, 76px result rows, 50px footer and 2px frame. Expanded height is 172 + 76 × rows; settings remains 620px. At 360px, navigation reflows and settings content scrolls while actions remain accessible.
- Colors: white #ffffff, text #181b20, secondary #606a78, selected #eceff3 and charcoal accent #20242a. Neutral dark/system counterparts retain readable icon and text contrast.
- Components: inset search field, underline filters, quiet row separators, contextual actions, sidebar settings, switches, native radio theme choices, keyboard-help disclosure and consistent dialogs.
- Assets: generated black/white search mark used in the interface, favicon, application PNG/ICO and 32px tray RGBA. The 16/24/32/48/64/128px icon sheet was inspected. File types use the existing Lucide library with semantic colors.

## Findings resolved

- [P2] Expanded search lacked the selected concept's field boundary. Added a subtle inset border and recaptured the final comparison.
- [P2] The footer duplicated the settings entry. Replaced it with the selected result's More Actions menu.
- [P2] The window click handler immediately dismissed that footer menu. Excluded its trigger from outside-click dismissal; verified the menu stays open and Escape dismisses it.

## Interaction verification

All/Files filters return four/three preview results respectively. ArrowDown changes selection; Shift+F10 and the footer open the result menu; Escape dismisses it. Pinning, clear-to-compact, settings navigation, theme save, keyboard-help disclosure and add-application dialog were checked. The 360px views have no horizontal overflow. Unmatched text presents 无匹配结果 and the Bing fallback. Browser console checks returned no warnings or errors.

## Intentional differences and limits

This is an implementation of the selected visual direction, not an exact pixel clone. Native window contracts are retained: the concept's normalized search/settings panels are approximately 527/728px high, while the actual four-row search/settings are 476/620px. The sidebar is narrower to preserve usable settings space. Generated title-bar window decorations are omitted because this is an undecorated launcher. Theme choices use recognizable Sun/Moon/Monitor library icons and accessible radio inputs.

The concept's filled Office/Adobe artwork is illustrative; consistent installed file-type icons are used instead. Generated font metrics and Windows DPI can cause minor glyph/subpixel differences (P3). No remaining actionable P0/P1/P2 visual finding was identified.

Browser preview verifies UI presentation and interactions; it does not exercise native OS dragging, multi-monitor behavior, actual application opening or installation/service lifecycle. Those native interactions were not retested in this redesign. No indexing/backend logic was changed.

## Build verification

- Frontend: 61 tests passed; Svelte/TypeScript check: 0 errors, 0 warnings.
- Rust: 86 tests passed, 1 existing opt-in probe ignored; desktop check passed.
- Production desktop build and NSIS packaging passed. Output executable hash matches the freshly built executable. Release manifest and SHA256SUMS were updated.

final result: passed

## Compact control polish (2026-10-14)

The prior compact-control revision used a small secondary-color “设置” text action with a 44×44 CSS px hit area. That text entry is superseded by the hover revision below. The shortcut badge and divider were removed; the configured launch shortcut remains available in the settings tooltip and settings page.

Fresh screenshots: `artifacts/archive/2026-10-19/outputs/Search-Bar-Polish-2026-10-14/compact-after.jpg`, `compact-dark.jpg`, `compact-narrow.jpg`. The browser preview verified 680×76 compact size, light/dark styles, 360px with no horizontal overflow, settings opening/theme save, and expanded budget-preview results. Browser warning/error log was empty. In-app preview font rendering may differ from the native Windows font environment.

Frontend/tooling: 70 tests passed. Svelte/TypeScript: 0 errors, 0 warnings. Frontend build, Rust release and NSIS packaging passed. The verified executable/installer and matching checksums are in `artifacts/archive/2026-10-19/outputs/Search-Bar-Polish-2026-10-14/`. This change modifies only search-bar presentation; prior engine/performance acceptance records remain historical evidence for their specific builds.

## Search icon hover settings (2026-10-14)

Per the user's correction, the right-side text action is removed. The left magnifying glass now switches to a 22px gear on hover or keyboard-visible focus, with a 140ms opacity transition and no input movement. Clicking opens settings. Its 44×44 CSS px button has no native drag attribute; negative horizontal margins preserve the original icon/input alignment. Reduced-motion rules disable the transition when requested.

Fresh browser verification: default icon opacities 1/0, hovered 0/1, pointer departure restores the magnifier; keyboard focus and clicking the entry work. Dark 360px view has no horizontal overflow and retains the 44×44 hit area; there is no visible text inside the empty search bar. Browser warnings/errors are empty. Evidence: `artifacts/archive/2026-10-19/outputs/Search-Hover-Settings-2026-10-14/default.jpg`, `hover.jpg`, `dark-narrow.jpg`.

70 frontend/tooling tests passed; Svelte/TypeScript has 0 errors and 0 warnings; frontend build, Rust release and NSIS packaging passed. The new executable/installer and verified checksums are in `artifacts/archive/2026-10-19/outputs/Search-Hover-Settings-2026-10-14/`. Browser screenshots may use different fonts from native Windows.
