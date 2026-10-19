# Silver launcher visual QA

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
