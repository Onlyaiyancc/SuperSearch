# SuperSearch extensions optimization

Date: 2026-10-22
Status: approved by the user and implemented; verification recorded in
`docs/design/qa.md` and `test-results/all-extensions/`.

## Goal and scope

Improve all four existing extensions in one bounded pass: translation and
dictionary, file preview, countdown reminders, and format conversion. Preserve
the existing neutral visual language and work on top of the current uncommitted
development. This pass improves interaction continuity, avoids redundant work,
and makes existing operations easier to understand.

## Evidence

- `src/App.svelte` keys both translation and reminder panels by the complete
  search query. Every character change destroys and recreates the panel.
- `src/TranslationPanel.svelte` initializes direction from the query at mount.
  Editing text therefore discards a manually selected direction.
- `src/ReminderPanel.svelte` reads reminders, subscribes to events and starts a
  one-second timer at mount. Editing a reminder command repeats that setup.
  The timer also runs while the mounted extension is disabled or has no pending
  reminders. Successful automatic refreshes do not clear an earlier read error.
- `src/FilePreviewPanel.svelte` reports text size using a fixed MiB formatter,
  which displays small segmented fixtures as `0.0 MiB`. Image/PDF metadata and
  read progress can be made clearer without adding native reads.
- `src/ConversionPanel.svelte` removes selected files one at a time. Its existing
  native `release_conversion_sources` command already accepts up to 100 IDs.

These are code inspection findings. Browser interaction verification will
establish the observable behavior before and after implementation.

## Approaches

1. Recommended: targeted changes within the existing panels and helpers. Keep
   native contracts, avoid new dependencies, and add regression coverage for
   asynchronous state transitions. This gives a small, reviewable change.
2. Broader alternative: introduce a common extension registry and shared panel
   lifecycle framework. That may help a future third-party plugin system, but
   adds migration work beyond the four current extensions' immediate needs.

## Proposed behavior

### Translation and dictionary

Keep the translation panel mounted while editing a command in the same mode.
Use query changes to invalidate results, copy notices and pending presentation,
with generation checks preventing late replies from restoring obsolete data.
Until the user changes direction, detect it from the current text; afterward,
preserve that selection for the current translation-panel lifetime. Switching
between translation and dictionary, leaving the panel, or hiding the launcher
ends that lifetime. Explicit Enter/button submission remains required.

Do not add query/result persistence, automatic uploads or another provider.

### Countdown reminders

Keep the reminder panel mounted while editing a reminder command. Reset only
the creation draft feedback when the query changes. Capture submission identity
so that completion of an older create request cannot mark a newer draft as
submitted. An active create/cancel operation continues to block duplicate work.

Coalesce list refresh bursts to one active read and at most one follow-up;
discard obsolete results after disposal. A successful list refresh clears the
prior list-read error without erasing unrelated create/cancel failures. Subscribe
once per enabled panel lifetime. Run the visual countdown timer only while the
panel is visible, enabled, and contains unfinished reminders. Resume uses the
actual deadline rather than accumulated ticks. Native scheduling remains
responsible for reminders when the window is hidden.

### File preview

Show concise metadata from the existing preview response: content type, readable
file size, PDF page position or text segment position as applicable. Use B/KiB/
MiB formatting so small files have a meaningful size. During navigation, retain
the previous successful content and expose a polite loading status. Preserve
existing retry, zoom, scroll and keyboard behavior; do not add prefetching,
unbounded caches or extra file reads.

### Format conversion

Add a `清空文件` action when files are selected. Disable it during draft mutation
or submission. Release the selected IDs in one native command; clear the draft
selection only after successful release, leaving it available for retry on
failure. Invalidate CSV preview state for the cleared sources. Preserve the
chosen output directory and compatible conversion preferences. Clearing a
selection does not cancel queued/running jobs or delete input/output files.
The backend already retains credentials referenced by active jobs.

## Verification and acceptance

- Editing translation text preserves a manual direction and clears stale
  results; resolving an old request cannot overwrite a newer query.
- Typing a multi-character reminder title keeps one event subscription and
  avoids per-character list reads. Editing during creation cannot duplicate
  submissions or show success against the wrong draft.
- Refresh bursts are bounded; successful reads recover from prior read failures;
  disabled, empty and completed-only reminder lists do not tick.
- Preview shows useful sizes for bytes, KiB and MiB; page/segment loading and
  failures preserve successful content and remain keyboard accessible.
- Conversion clear sends one batch of IDs, preserves draft on failure and
  retains destination/options and ongoing tasks on success.
- Add regression tests before behavior changes. Run frontend tests, Svelte/
  TypeScript validation, frontend production build, Rust tests/check, and browser
  interaction/visual checks for all four panels in the current light/dark style.
- Use mocked native replies for browser tests; do not send translation text,
  create real notifications, download components or convert user files.

## Baseline verification

Fresh checks on the existing working tree, before implementation:

- `npm test`: 20 files, 280 tests passed.
- `npm run check`: zero errors and zero warnings.
- `npm run desktop:check`: passed.
- `npm run desktop:test`: final run passed 222, ignored 3.

Sandbox execution initially prevented frontend config resolution and caused
native directory/network fixture errors. Frontend and native tests were rerun
with reviewed escalation. The first native run outside the sandbox failed one
unrelated Everything loading-to-ready timing test; its isolated rerun and the
subsequent full run passed. Record this as a transient baseline failure, not an
extension regression or proof that the timing test is always reliable.

## Boundaries

No new extension types, external services, component packages, settings schema,
plugin framework, commits, release publication or installer changes are included
in this proposal. Existing uncommitted files remain part of the working tree.
