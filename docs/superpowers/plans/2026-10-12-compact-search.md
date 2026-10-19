# Compact search revision

The user's selected screenshot is the visual target. Latest instruction overrides previous language guidance: all visible interface copy is Chinese; original app/file names and technical identifiers stay intact. Keep Chinese code comments and English diagnostic output.

## Accepted behavior

- Every search opening starts a fresh session: blank query, All scope, cleared selection, stale requests cancelled, focused input.
- Idle/no-match UI contains only the rounded search bar and shortcut badge. Nonblank searches with current results expand downwards. No empty history list or permanent footer.
- Previously successfully opened matches rank before unused matches, respecting scope, terms and path/extension/privacy filters. Do not turn unrelated history into results. Deleted files are excluded. History recording off/clear removes its influence.
- Native window starts at 918×112 logical pixels. Minimum 360×112; compact/results/settings layout handled natively, bounded to monitor work area. Expanded visible rows <=6 at 60px; results height 206 + 60×rows; settings/dialog height 620. Keep top anchor where possible. Sync IPC contract: `sync_window_layout({ layout: 'compact'|'results'|'settings', rows: number })`. Search openings reset compact before show; layout IPC may resize only when visible.
- Settings via tray, Ctrl+, and search icon's explicit context menu. No permanent header/gear in idle state. Chinese IME, keyboard selection, dark theme and reduced-motion remain supported.

## Tasks

1. Backend: failing regression tests first; revise ranking and history candidate retrieval; skip empty-query enumeration; implement bounded native sizing/reset and localize tray. Own Rust source/config only.
2. Frontend: failing state/contract tests first; implement Chinese UI, compact conditional results, full reset, layout sync, restrained transitions and responsive styles. Own src/ and tests/ only. Native command contract above.
3. Integrate: verify frontend/Rust checks, browser UI states and screenshot comparison with source; spec review then independent quality review; fix actionable findings and repeat verification.
4. Build replacement executable/installer, verify packaged artifacts, refresh user-facing docs/checksums and preserve honest verification limits.

## Visual acceptance

Ivory canvas, white 80px search surface, 16px outer spacing, 15px corner radius, thin warm border, gold outline magnifier, natural Chinese font with muted placeholder, discreet Alt+Space badge. No decorative imagery or extra product features. Evaluate typography, whitespace, hierarchy, contrast, transitions, focus/hover, narrow viewport, dark mode, and reference fidelity. Record evidence and remaining limits in docs/design/qa.md.
