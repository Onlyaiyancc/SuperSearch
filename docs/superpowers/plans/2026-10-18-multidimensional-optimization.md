# Multidimensional optimization implementation plan

**Goal:** Preserve application aliases during editing, bound file-merge work, and stop pagination when an index returns an empty page.

**Architecture:** Retain existing ranking, limits, Chinese UI and silver theme. Extend the result DTO with optional aliases. Preserve unchanged alias labels including legacy commas. Use a temporary set of kind/ASCII-folded locator keys for each merge, preserving first-entry order and the 1,000-file cap. Clamp pagination total to the consumed offset on a successful empty page; failed pages remain retryable.

**Tech stack:** Rust, Tauri 2, Svelte 5, TypeScript, Vitest. Execute inline in the existing dirty checkout, preserving prior work. No publication or commit required.

## Tasks

- [x] Add failing alias round-trip tests in `tests/frontend-settings.test.ts` and serialized snapshot coverage in `src-tauri/src/service.rs`.
- [x] Add alias payload in `src-tauri/src/model.rs` / `service.rs`, optional TS field in `src/types.ts`, prefill in `src/App.svelte`, and unchanged-draft handling in `src/settings-input.ts`.
- [x] Add empty-page regression and error/retry coverage in `service.rs`; fix successful zero-progress pagination without deleting existing results or changing supplemental counts.
- [x] Add merge equivalence coverage (ASCII case, non-ASCII distinctions, kinds, duplicates, first-entry identity, cap) and an opt-in release workload comparing the prior linear algorithm with the new set implementation.
- [x] Run focused tests red/green, full frontend and Rust checks, production build, dependency audits, isolated bundled smoke and browser dialog checks. Inspect UI screenshots against `docs/design/qa.md`.
- [x] Review final diff and record fresh results, component-only performance evidence, and remaining native interaction limitations in `artifacts/archive/2026-10-19/outputs/Optimization-Report-2026-10-18.md`.

## Alternatives considered

Broad architecture changes increase risk without a measured bottleneck. A visual redesign conflicts with the established design direction. Focused fixes provide testable benefits in correctness, interaction, performance and maintenance while keeping the existing product behavior.
