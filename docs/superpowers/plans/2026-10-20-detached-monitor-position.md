# Detached monitor position implementation plan

**Goal:** Complete the approved remembered-position behavior: when its display disappears, center on the remaining target display instead of retaining invalid or OS-relocated corner coordinates. Keep ordinary dragging and cursor-follow behavior.

**Cause:** The existing fit clamps stale off-display coordinates to work-area edges. Windows can also move the HWND to a corner before recall, so checking only the current position loses the evidence of disconnection.

**Approach:** Keep the last successfully used monitor bounds in process memory. Observe native move events to follow deliberate cross-display dragging while that previous monitor is still connected. If it disappears, retain that fact through OS relocation, clear the position anchor during layout recovery, and adopt the new monitor only after successful layout. Enumerate displays only on layout/move demand; no polling or stored-settings migration.

- [x] Add deterministic tests for disconnect before/after OS relocation, normal cross-display dragging, unrelated display removal and successful recovery adopting the new display. Confirm the regression fails (two assertions failed before the correction).
- [x] Add the small monitor-memory policy in layout.rs; wire native move events and shared layout application in service.rs/main.rs.
- [ ] Run all frontend/Rust checks, release build and native diagnostics. Obtain independent read-only code review and Windows CI.
- [ ] Deliver a separately hashed third candidate with a short Chinese B-only recheck. Preserve the user's A/C and size passes as user-reported evidence for candidate 2; installer cases stay deferred and no public release is created.
