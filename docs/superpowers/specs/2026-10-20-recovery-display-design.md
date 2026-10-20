# Candidate recovery and display behavior

## Accepted scope

The user reported successful initial index readiness, IME, shortcut/single-instance, settings persistence and sleep/wake checks. Manual engine restart displays a terminal unavailable status. Unplugging the secondary monitor leaves the recalled window at an incorrect size until process restart. Interactive installer/UAC and missing-WebView2 checks remain deferred. These findings supersede any assumption that automated candidate acceptance established these physical interactions.

The user confirmed: add two display-position choices, retaining the current remembered-position default and allowing each recall to center on the monitor containing the mouse pointer. Reposition only on recall; typing, expanding results and opening settings within the visible window retain the current anchor. Following the active application was considered and rejected by the user in favor of following the cursor.

## Implementation

- Engine status: preserve the known loading state when a bounded status request times out during restart. Treat a busy state check as transient loading; genuine worker/launch failures remain unavailable. Retain bounded synchronous replies, no locks while waiting, and no background idle polling.
- Window geometry: use the canonical 680 logical-pixel width on every layout application, clamped to the destination work area. Never derive the intended width from potentially stale physical dimensions/DPI. On DPI changes, force the current frontend layout through the existing serialized synchronization path even if layout/row keys are unchanged. Hidden windows are normalized at the next recall.
- Settings: add validated `windowPlacement` values `remember` and `cursor`, with a backward-compatible missing-field default of `remember`. Add a visible Chinese label and native select on the general settings page, using existing neutral styling. Apply the saved selection at recall without restarting the application.
- Verification: deterministic restart-timeout and busy-mailbox tests, negative genuine-error cases, native manual-restart smoke, fixed-width/DPI/clamping/placement tests, forced layout coalescing tests, settings round-trip/validation checks, full frontend/Rust checks, build and targeted visual/native diagnostics.

## Release boundary

Prepare a new candidate revision under a separate artifacts directory, preserving the original candidate and its evidence. Do not publish a public release or stable tag. Previous manual passes remain user-reported historical evidence; corrected restart and monitor-unplug behavior require fresh verification on the replacement build. Do not require a further design confirmation for the behavior already confirmed in this chat.
