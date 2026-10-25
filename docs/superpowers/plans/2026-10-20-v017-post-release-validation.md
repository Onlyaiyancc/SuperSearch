# v0.1.7 post-release validation plan

**Goal:** Execute the requested release resource diagnostic, extend available Windows/DPI evidence, and collect actual user feedback for the next release.

**Approach:** Test the published portable executable with SHA-256 `46d48ca0af796b13e46c38bbf22c12c875dc8fca6aa99177f282a0f1419ac1b2`. Use existing isolated diagnostic modes. Keep raw evidence in `test-results/v0.1.7-followup-2026-10-20/` and archive sanitized results under `artifacts/releases/GitHub-v0.1.7/Followup-2026-10-20/`. Do not change released binaries or infer physical acceptance from synthetic tests.

## Resource diagnostic

- [x] Verify the published ZIP and extracted executable against publication digests.
- [x] Preserve existing root soak reports before running the existing collector.
- [ ] Run `pwsh -NoProfile -File scripts/soak.ps1 -Executable test-results/v0.1.7-followup-2026-10-20/portable/SuperSearch.exe -DurationSeconds 1800`.
- [ ] Inspect native outcome, duration, query/window failures, process-tree memory change/slope, latency and raw-sample/report digests.
- [ ] Archive the exact raw samples and reports with a manifest.

## Windows and DPI

- [x] Identify the available host as Windows 11 Pro build 22621; user reports no Windows 10 test machine.
- [ ] Check available native UI control and display settings; record the original scaling before any reversible change.
- [ ] Where available, execute the published native diagnostic at 150% and 200%, capture actual geometry and restore original scaling. Keep the fixed-scale resource run free of intentional display changes.
- [ ] Record unsupported Windows 10, physical monitor hot-plug, IME or sleep scenarios as unverified unless actually observed.

## Feedback and prioritization

- [x] Ask the user for real usage observations: user reports no current issues.
- [ ] Read public repository issues if access is available and retain source links and retrieval time.
- [ ] Record feedback without treating absence of reports as proof of correctness.
- [ ] Publish a local follow-up report distinguishing completed tests, environmental limitations and evidence-based next priorities.

## Final verification

- [ ] Recompute archived evidence hashes, check all report links, inspect `git diff --check` and confirm only intended records changed.
- [ ] Report completed and unavailable items accurately; no release publication or communication to third parties is included.
