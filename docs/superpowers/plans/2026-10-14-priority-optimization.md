# Priority Optimization Implementation Plan

**Goal:** Execute the approved P0/P1/P2 roadmap with reproducible evidence and deliver a uniquely identified build.

**Architecture:** Extend the existing isolated native UI benchmark with a soak mode and bounded lifecycle cycles. Collect per-role process samples externally. Preserve established history-first ranking while adding a representative quality corpus and addressing measured allocation or computation bottlenecks. Use independent workers for benchmark and ranking modules, with integration and release owned by the primary agent.

**Tech Stack:** Existing Tauri 2, Rust, Svelte 5, TypeScript, Node and PowerShell; no runtime dependencies added.

## P0: Resources and native stability
- [x] Add failing tests for soak parameters, completion, bounded workload, hidden/visible transitions and failure reports.
- [x] Implement explicit `--ui-soak` with 30-minute duration, isolated store/engine/WebView, startup diagnostics, repeated search and native hide/show cycles. Keep normal mode free of timers.
- [x] Implement external resource collector with per-role Private Bytes, working set and CPU deltas, PID/start-time identities, streaming samples and fresh executable hash.
- [x] Build production executable and run a 30-minute soak; report CPU sampling limitations and resource trends without claiming physical IME or multi-monitor coverage. Final hash: 60 cycles, 120 searches, 640 samples; peak Private Bytes 243.11 MiB and initial/final-quarter median growth 14.26 MiB. No leak-free claim.
- [x] Review native lifecycle/input/layout edge cases; reproduce and regression-test any concrete issue before fixing. Fixed demand-driven service restoration and moved provider status IPC off the UI command thread. Non-focusable/inert long diagnostics prevent interference from manual desktop input.

## P1: Quality and scale
- [x] Add a representative deterministic query corpus, top-one/top-three metrics and scope/path cases; preserve the documented history-first behavior.
- [x] Measure existing ranking and optimize repeated work only with benchmark evidence, bounded cache behavior and invalidation tests.
- [x] Add a configurable isolated large-file fixture benchmark and run at least 100,000 fixtures, with separate setup/index/query timing and cleanup limited to its owned root.

## P2: Release maintenance
- [x] Establish a source baseline using Git if available; never include caches, user data, build trees or historical binaries in source history. Initial local source commit: a3b3103.
- [x] Add atomic release packaging with executable hashes, version consistency and a single latest-delivery descriptor; test stale/mismatched inputs. Review also added source/installer build guards and native/raw evidence digest pairing.
- [x] Run Rust dependency audit through the local wrapper; record actionable findings and dependency constraints.
- [x] Run `npm run verify -- --smoke`, production build and native acceptance on the final hash; preserve artifacts and update README/AGENTS with actual results. Final checks: 142 frontend/tooling, 107 Rust passed plus one opt-in ignored; static checks/build/NSIS ownership/smoke/npm and Rust audits passed. Published 0.1.6 to artifacts/releases/Priority-Optimization-2026-10-15 with paired evidence and Latest-Release.json. Manual native acceptance limits remain documented.

## Validation and limitations
Workers own disjoint files and must not revert another worker's changes. Primary agent reviews each patch and performs final integrated checks. No test modifies a user's personal Everything instance, search database or existing install. Installation/UAC cancellation, physical IME, sleep/wake and multi-monitor cases require suitable native test environments; document actual coverage separately from simulated assertions. Changes are backed up before editing. User approval of the roadmap authorizes this execution; no additional workflow approval is needed.
