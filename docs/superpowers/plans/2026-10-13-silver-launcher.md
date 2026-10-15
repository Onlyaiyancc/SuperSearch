# Silver Launcher Implementation Plan

**Goal:** Implement the user's selected first visual concept across the existing desktop app.

**Architecture:** Preserve Svelte search orchestration and Tauri IPC. Replace visual tokens and presentation, using the existing 680px native window and 76px row contract. Generate the selected brand mark as a raster source and derive Windows assets from it.

**Tech Stack:** Svelte 5, TypeScript, Lucide, Tauri 2, Pillow.

- [x] Replace `src/styles.css`: white/silver/black tokens, neutral dark counterpart, 76px compact header, 46px filters, 76px rows, 50px footer, 620px settings.
- [x] Update `src/App.svelte`: simple input, left-aligned underline filters, understated result actions, sidebar settings, accessible theme radio choices, keyboard help disclosure, consistent dialogs.
- [x] Update `src/ResultIcon.svelte` with distinct file-format library icons and `src/BrandMark.svelte` with the generated image.
- [x] Add clearly labeled design preview samples to `src/bridge.ts`, keeping native preview boundaries intact.
- [x] Convert the generated mark through `scripts/build-icons.py` into PNG, multi-resolution ICO, and 32px RGBA tray assets; update favicon.
- [x] Run frontend tests, Svelte validation, Rust tests/check and production desktop build. Existing tests cover IPC, IME, search state and layout; verify presentation changes through real browser interaction.
- [x] Compare browser captures to `outputs/Silver-UI-QA/reference.png`; check 680px and 360px layouts, themes, settings, menus, dialogs and keyboard use. Record evidence in `design-qa.md`.

The workspace has no Git metadata. Preserve pre-change source copies in `work/silver-redesign`; do not create a repository or commit unrelated files.

