# WebView2 cloud acceptance plan

**Goal:** Complete the remaining dependency-install acceptance without requiring the user to reboot, install a VM or change their host runtime. Reuse the exact successful main Windows artifact and existing disposable-runner identity/checksum guards. No application changes or public release.

- [x] Add tests proving ordinary hosts, self-hosted runners and non-Windows environments are rejected before mutation; observe red then green.
- [x] Append a small cloud-only helper to the existing installer lifecycle protocol. Verify real runtime absence, installer-scoped download failure/no partial installation, online retry/runtime restoration and real native WebView rendering.
- [ ] Run tooling tests, syntax checks, audit and read-only code review. Dispatch on a branch against main build 39364003496 / 651580dd56a157449a46aca43b350b45482beb7c.
- [ ] Review cloud evidence and any failures, preserve exact provenance/limitations, and integrate the reusable acceptance protocol after checks pass. Keep previous candidate deliveries unchanged.
