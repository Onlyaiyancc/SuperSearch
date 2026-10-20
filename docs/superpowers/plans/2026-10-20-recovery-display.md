# Recovery and display implementation plan

Execute inline using the approved design in the adjacent spec; do not publish a release.

- [x] Reproduce the restart status timeout with a test using the actual EverythingClient API, and preserve terminal failures in a negative case. Add the actual retry/status sequence to the bundled diagnostic.
- [x] Fix the status timeout/busy transitions with the smallest shared change; verify failed tests become green. Include bounded startup SDK-timeout grace discovered in review.
- [x] Add fixed-width geometry, placement policy and forced layout synchronization tests before implementation.
- [x] Add the validated remembered/cursor placement setting in model/store/frontend, and fix DPI/unplug sizing and current-layout resynchronization.
- [ ] Run frontend and Rust checks, notices/audits, native smoke and build. Review the final diff, submit a PR and verify remote checks.
- [ ] Package a separately hashed replacement candidate. Record the previous candidate's user-reported failures and identify the two physical rechecks needed on the replacement. Leave installer special-case tests deferred as requested.
