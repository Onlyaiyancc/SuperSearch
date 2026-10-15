# SuperSearch UI and application icon redesign

**Goal:** Apply the user-requested UI UX Pro Max and Anthropic Frontend Design skills to the existing desktop app, including its application icon.

**Context:** Both skills were installed from the user-provided repositories into Codex user skills. The supplied directory page for frontend-design points to anthropics/skills; its current official version was installed. Product subject is already clear: a compact, offline, keyboard-first Windows search launcher. Existing requirements remain: Chinese UI, blank 680×60 search bar, expand only for matching results, reset on recall, light/dark themes.

**Design:** Cool porcelain #f4f6fa, white #ffffff, ink #1b2b45, secondary #58667b, cobalt #275dde, selected blue #eaf0ff. Typography uses locally installed Segoe UI Variable for Latin and Microsoft YaHei UI for CJK; no font download or network dependency. The memorable element is a precise cobalt target/lens mark, shared by app icon and search button. Controlled density: clear name/path hierarchy, restrained result selection, coherent form surfaces, and a quiet navigation rail. Keep the search input as the entire idle UI.

**Skill calibration:** The generic generated system returned a landing-page/demo pattern, inappropriate for a desktop launcher; it is not used or persisted. A narrower style query verified Fluent 2 platform guidance. We adopt desktop hierarchy/focus principles and a custom palette. The Svelte keyboard search returned no matches on one retry; existing tested Svelte behavior and general accessibility guidance are the fallback.

- [ ] Preserve before-state screenshot and CSS/native icon backup.
- [ ] Create repo-native SVG source plus PNG/ICO/tray exports; validate small sizes and transparency.
- [ ] Implement compact search, result list, settings, menu, and modal visual tokens with existing behavior.
- [ ] Check light/dark, idle/results/settings/modal, keyboard focus, 360/680px, text clipping, and reduced motion.
- [ ] Fix verified indexing installer lifecycle issues and explicit reparse-point coverage.
- [ ] Run existing regressions, compile native release, install/update, verify NTFS service and ship one coherent final release.
