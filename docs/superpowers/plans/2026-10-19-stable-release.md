# v0.1.7 release preparation plan

Goal: prepare a separately versioned candidate and reviewable evidence without publishing it.

Architecture: reuse the existing installer acceptance, build wrappers and release workflow. Keep the product behavior and dependency versions stable. Execute inline in the current workspace; preserve the approved scope in the adjacent design document.

- [x] Add executable checks for release tag classification, exact version agreement and safe draft behavior; demonstrate failure before implementation.
- [x] Update the release workflow to use that validation and keep stable releases as drafts.
- [x] Remove the fixed candidate version from installer acceptance. Derive it from the exact candidate commit, verify manifest and PE versions, and require an actual version increase over the pinned baseline.
- [x] Record initial-index milestones and failure state in the fast native diagnostic without changing its deadline or application search behavior.
- [x] Synchronize all five version declarations to 0.1.7; update candidate documentation and publish instructions.
- [ ] Run frontend/Rust verification, audits, notices, build and isolated release smoke. Hash the resulting installer and portable package.
- [ ] Submit reviewable source through a branch/PR when the connected GitHub account permits it; use the hosted workflow for destructive installation checks only.
- [ ] Record current-candidate resource evidence and unresolved physical acceptance requirements. Leave public release/tag creation for a subsequent user decision.

Validation commands: `npm test`, `npm run verify -- --smoke --audit-rust`, `npm audit`, `npm run notices -- --check`, `npm run desktop:build`. Installer acceptance remains guarded by the disposable hosted runner checks in `scripts/native-installation.ps1`.
