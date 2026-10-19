# Beta native acceptance plan

**Goal:** Validate the exact Windows CI candidate before physical IME/multi-monitor acceptance and publication.

**Architecture:** Reuse the successful main-branch build artifacts. A manually dispatched, read-only workflow executes installer lifecycle tests on a disposable GitHub-hosted Windows runner. Local regression and engine tests run against the same candidate. Physical UI acceptance follows only after automated checks pass.

- [ ] Add a hosted-runner-only installation harness and synthetic SQLite persistence fixture.
- [ ] Validate fresh installation, beta.1-to-candidate replacement, dedicated service/NTFS queries, shortcut, retained preferences/aliases/pins and uninstall coexistence.
- [ ] Run existing IME, layout, provider-refresh, engine and bounded resource checks; retain exact hashes and reports under `test-results/native-acceptance/`.
- [ ] Review changes and record CI outcomes without claiming physical interaction coverage.
- [ ] Use Computer Use for real Microsoft Pinyin, shortcut and cross-monitor interactions.
- [ ] Publish the accepted candidate only when required checks have passed; record remaining limits explicitly.
