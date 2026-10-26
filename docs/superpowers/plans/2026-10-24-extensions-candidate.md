# Complete extensions candidate

**Goal:** Stabilize the known native test race and deliver a locally verified candidate containing the latest preview, translation, reminder and conversion changes.

**Execution:** Continue in the existing `codex/translation-dictionary` working tree because it contains the complete uncommitted extensions. Preserve that work. Use the existing project wrappers and isolated diagnostic modes. Public release and machine installation are separate from this local delivery.

## Native test race

- [x] Reproduce the unchanged test: the isolated test failed on attempt 11 with `loading` instead of `ready`, followed by the fixture worker timeout.
- [x] Compare restricted and normal Windows execution: directory-handle and loopback cancellation checks fail only under the restricted harness; the unrestricted full baseline passed 222 tests, with three opt-in diagnostics ignored.
- [x] Replace test worker mailbox polling with its real Windows wake event. Own the event with `OwnedHandle`, await a bounded signal, publish ready, release the mailbox, then disconnect the reply. Production status behavior is unchanged.
- [x] Run the targeted wrapper test and 200 fresh-process repetitions, then the complete default parallel native suite with normal Windows access: 222 passed, three ignored.

## Integration review correction

- [x] Read-only integration review found that conversion Escape handling intercepted settings dismissal.
- [x] Add a production-browser scenario for conversion query/settings/Escape and selected-file draft retention. Observe settings remaining open before the fix, then move settings dismissal ahead of conversion handling in `src/App.svelte`; the same scenario passes afterward.
- [x] Preserve the first candidate under the `-initial` directory and its interrupted soak samples as non-acceptance evidence. Rebuild and restart diagnostics against the corrected executable.

## Candidate and evidence

- [x] Run frontend tests, type validation/build, native check and dependency audits; record exact outcomes. Revalidate after the Escape correction.
- [x] Obtain independent read-only review of this test fix and candidate preparation; review the corrected integration finding.
- [x] Build through `npm run desktop:build` with the existing isolated `.tools/conversion-release-target` so a running older executable does not block output.
- [x] Copy the new executable, installer, licenses and exact pinned local component ZIPs into `artifacts/releases/All-Extensions-Candidate-2026-10-24/`; record 207 source inputs, a verified source snapshot and SHA-256 checksums.
- [x] Run a fresh bundled-engine smoke and short native UI/resource diagnostic against the copied executable. The corrected candidate completed 30 minutes, 60 cycles and 742 resource samples. Private-memory first/last-quarter medians changed by +6.32 MiB; this does not establish absence of leaks.
- [x] Write the local candidate instructions, acceptance results and remaining physical interaction/public component publication limits. Verify all 85 delivery checksums and inspect the final diff. Final checks passed 282 frontend tests, 222 default parallel native tests, 29 browser checks and the separately opted-in real adapter probe. Rust audit retains six upstream warnings; notices match 373 packages.
