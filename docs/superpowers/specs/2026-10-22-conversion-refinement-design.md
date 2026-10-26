# Conversion workflow refinement

Date: 2026-10-22. Status: all three refinements approved, implemented and locally verified.

## Context and recommendation

The local conversion extension and representative real engines already pass their initial acceptance. This round targets defects in the existing selection, retry and refresh flows, preserving the agreed format matrix, local processing and component installation model.

Three approaches were considered:

1. **Repair workflow correctness and refresh efficiency (recommended).** Resolve reproducible state defects with small changes to the existing panel, state helpers and native queue.
2. Add formats and conversion presets. Useful later, but introduces new engines, packages and compatibility acceptance before improving existing operations.
3. Redesign the panel and add a history manager. Broader UI scope; persistent history also conflicts with the existing session-only privacy model.

## Proposed behavior

### Valid options after selection changes

Adding a non-image source to an image-to-PDF merge currently leaves `merge=true`, although the checkbox disappears. The hidden stale option prevents submission. Normalize source-dependent options whenever the selection changes or a saved draft is restored. Preserve compatible user choices; clear merge when the target is not PDF or the selected batch is not all images. Empty selections continue to retain the draft for later re-addition.

### Retry retains original task intent

The existing per-item retry copies target/options but uses the current draft output directory and forces `merge=false`. Changing the draft can silently redirect a retry; retrying a merged batch can turn it into a single-image PDF.

Add a native retry command identified only by the existing job ID and optional failed source ID. The backend retrieves the original sources, file identities, options and output grant from its bounded session job list. No frontend paths or new file permissions are accepted. Only terminal failed work is retryable. Normal retries submit the failed item; merged PDF failures retry the original ordered image batch as a whole, with an explicitly labeled batch retry action. Completed outputs remain intact and collision handling remains unchanged. Reuse the normal queue validation, limits and source identity checks. A missing/changed source or expired task produces a localized, actionable message. CSV retries reuse the reviewed original encoding/delimiter only for the unchanged granted source.

### Bounded, event-specific status refresh

Currently each conversion event refreshes both jobs and components, and overlapping reads are only discarded after execution. Keep one request in flight per resource and coalesce a burst into at most one pending follow-up. Publish the latest state, suppress disposed replies and retain retry after errors. Job events request jobs only; component events request components only. Manual refresh and initial mount refresh both. No interval polling or persistent cache is introduced.

## Scope and validation

Use existing Svelte controls, native grants and queue helpers. Keep Chinese UI and code comments, neutral light/dark styling, current component versions and package hashes. No new dependencies, formats, network services, automatic installations or persistent conversion history.

Regression tests cover mixed-source merge normalization and compatible draft retention; normal/merged retries retaining original destination/options/order and rejecting invalid job states; queue limits and replaced-source rejection; refresh burst coalescing, latest reply behavior, failure recovery and disposal. Run the full frontend suite/check/build and native suite/check, then build a separate local candidate and verify its checksums/startup. Representative real engine smoke is repeated if engine adapter behavior changes. Preserve unrelated preview/reminder/translation edits and the running old application.

Acceptance requires all new regressions and appropriate existing checks to pass. Package the optimized candidate separately from the initial conversion delivery. Public publication and installation remain separate actions.
