# Security policy

SuperSearch is a Windows desktop application. Security fixes target the latest published stable release, currently [v0.1.7](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.7). Older stable builds and pre-releases may require an upgrade.

## Reporting a vulnerability

Use the repository's **Security → Report a vulnerability** facility when available: https://github.com/Onlyaiyancc/SuperSearch/security/advisories/new

If private reporting is not enabled, open a minimal issue asking the maintainer to establish a private reporting channel. Do not post exploit payloads, credentials, personal file paths or sensitive diagnostic logs in a public issue.

Provide the affected version, Windows version, installation mode, impact, minimal reproduction and any suggested fix. Do not test against other users' computers or data. There is no guaranteed response SLA.

## Trust boundaries

The search UI runs as a normal user. The machine installer requests administrator permission to install a dedicated indexing service named `Everything (SuperSearch)`. The bundled engine and third-party notices are identified in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

Release builds are currently unsigned. Obtain binaries from this repository's Releases page, verify published checksums, and review installer prompts. Signing certificates, tokens and private keys must never be committed.
