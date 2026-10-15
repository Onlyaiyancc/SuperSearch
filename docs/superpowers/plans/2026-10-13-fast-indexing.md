# Fast indexing implementation plan

**Goal:** Enable the approved bundled Everything service + hidden client architecture with NTFS/USN indexing.

**Architecture:** The machine installer puts the signed vendor service executable in Program Files and installs a dedicated `SuperSearch` Everything service instance and pipe. The ordinary-user client keeps its private database, uses fixed NTFS volume indexing when that service runs, and scans accessible folders otherwise. Search include/exclude filters remain in the existing query compiler.

**Tech stack:** Existing Rust/Tauri/Svelte stack, vendor Everything 1.4.1.1032, Windows SCM, NSIS hooks. No additional dependencies. Existing Everything installations are never started, stopped, configured, or removed.

- [ ] Add failing regression tests for fast INI mode, folder fallback, and folder partitioning.
- [ ] Implement dedicated service detection and mode selection in engine.rs and bridge.c. Keep a forced folder mode for isolated diagnostics.
- [ ] Package official EXE and license as installer resources; use perMachine installation and scoped service install/upgrade/uninstall hooks.
- [ ] Add an NTFS smoke diagnostic checking fresh create/rename/delete, isolation, and query latency; retain folder diagnostic.
- [ ] Update status translations and setup instructions; bump release to 0.1.5.
- [ ] Run Rust/frontend checks, native diagnostics, build installer; inspect generated NSIS service ownership and artifact hashes.
- [ ] Install via UAC, verify dedicated service path/state, restart the new ordinary-user application and report actual mode.

The user approved this architecture in referenced chat 2 and requested activation here. This directory has no Git repository, so commits/worktrees do not apply. Installation is authorized; Windows UAC consent remains an OS requirement. All process termination is limited to the observed SuperSearch process and children it owns.
