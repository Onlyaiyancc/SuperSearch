# Open-source release preparation

User approved the open-source preparation scope discussed in this chat. Execute inline, preserving existing implementation and folder cleanup changes.

1. Prepare MIT licensing for original code, complete third-party notices, security/contribution guidance and GitHub issue templates.
2. Keep the existing local Rust toolchain preferred; support a standard installed Rust toolchain when `.tools/` is absent. Add failing launcher tests first, then implement and verify. Document Windows prerequisites and fresh-clone commands.
3. Replace local-only README links with public documentation and GitHub Releases links. Keep public screenshots and essential evidence in tracked docs; exclude local caches, credentials, certificates and generated schemas.
4. Add Windows CI (frontend/Rust tests and checks, build and isolated smoke) and an explicitly dispatched Pre-release workflow; pin external actions to verified commits and minimize workflow permissions.
5. Scan candidate files and complete Git history for secrets, check dependency/license notices, validate a clean export without `.tools/`, and run fresh verification.
6. Review the full candidate diff; create a source commit and a matching release build. Publish to `Onlyaiyancc/SuperSearch` if account access permits, then verify remote contents/CI and release asset hashes. Do not expose credentials, rewrite unrelated history or claim remote verification without tool evidence.

License/repository defaults are MIT / `Onlyaiyancc/SuperSearch`; the user may steer these while preparation proceeds. Do not publish before the preparation and validation steps complete.
