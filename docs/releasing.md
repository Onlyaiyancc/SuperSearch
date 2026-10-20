# Release process

Public downloads are hosted on [GitHub Releases](https://github.com/Onlyaiyancc/SuperSearch/releases). `artifacts/` and test outputs are local maintainer records and are not committed.

## Prepare and publish

1. Update the versions in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` together, including lockfiles and `docs/releases/<tag>.md`. Use `vX.Y.Z-beta.N` for beta tags and `vX.Y.Z` for stable tags; the numeric version must match the source.
2. After dependency changes, run `npm run notices`. Record candidate results and unverified scenarios using the [native acceptance checklist](testing/beta-native-acceptance.md). Include executable/installer versions and hashes, license files and actual signing status. Exclude private keys, certificates and personal diagnostics from commits.
3. Push reviewed source to main and wait for **Windows checks and pre-release**. It scans Git history, installs locked dependencies, checks notices, runs frontend/native tests and static checks, builds Windows packages and performs isolated engine diagnostics.
4. After candidate acceptance and authorization, manually dispatch the workflow with a tag. Publication write permission is granted only to a tagged manual dispatch. Beta tags publish a pre-release; stable tags create a draft. Preparing a stable draft does not authorize public publication.
5. Review the draft's exact source target, notes and assets. Assets include the installer, portable ZIP with license files and SHA-256 checksums; attach a sanitized acceptance report when available. Publish a stable draft only after explicit maintainer authorization. Confirm stable/pre-release and latest status, download the public assets again, compare checksums and verify the tag's source commit.

Use English for release titles and descriptions. Describe the changes, installation/upgrade steps, signing status, administrator/service requirements, WebView2 dependency and remaining validation limits. Do not rewrite or retag published binaries to change documentation alone. Keep English and Chinese README download instructions consistent.

## Current release

[SuperSearch v0.1.7](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.7) was published as the latest stable release on 2026-10-20. It is unsigned and points to application source `651580dd56a157449a46aca43b350b45482beb7c`. The installer, portable ZIP, acceptance report and checksums were downloaded after publication and verified against the accepted draft.

See [v0.1.7 notes](releases/v0.1.7.md) and its attached `Release-Acceptance.md` for the actual evidence scope. A public release and a signed release are separate decisions; signing keys belong in a secure signing service or encrypted GitHub secret, never Git history.

## Candidate acceptance

- Build with the tag input empty. Record the successful main build's full source SHA and run ID, then dispatch with `native_acceptance`, `candidate_run` and `candidate_sha`, keeping tag empty. Acceptance reuses the original build assets and verifies source versions, PE versions and file hashes.
- The current upgrade protocol uses the public 0.1.6 beta.1 installer and its fixed hash as a baseline. A stable candidate must have a newer internal version; same-version replacement is not cross-version upgrade evidence. Update the baseline deliberately when appropriate.
- `--smoke-test-fast` records first engine observation, first provider readiness, first fixture visibility, query errors and last state in `initialIndex`. Engine `startupMs` measures extraction/startup, not whole-disk indexing. Observation includes polling and IPC delays. Preserve failed reports instead of replacing cold-index timeouts with a successful retry.
- Bind installation, resources and physical interaction evidence to the same candidate. Record signing, display hot-plug, sleep/wake, 150%/200% DPI, UAC and missing-WebView2 results individually. Older beta results are historical evidence, not acceptance of a new binary.

The repository owner should enable private vulnerability reporting, Dependabot alerts and available secret scanning/push protection, then require successful CI for main-branch merges. These settings may require repository administration rights.

The stricter local archive flow remains available through `npm run release -- --output artifacts/releases/<new-directory> --with-diagnostics`. It requires additional long-duration and scale diagnostics; do not substitute reports from an older build.
