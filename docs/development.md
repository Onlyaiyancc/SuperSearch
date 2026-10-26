# Development and builds

## Windows setup

The native target is Windows x64. The backend depends on Windows APIs and the Everything SDK; native tests cannot run directly on Linux or macOS.

1. Install Git and [Node.js](https://nodejs.org/) 24 LTS. The frontend requires at least Node.js 22.12; the locally validated version was 24.11.1.
2. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with **Desktop development with C++**, MSVC x64 tools, the Windows SDK and **C++ CMake tools for Windows**. The screen QR decoder builds its bundled C++ core. The desktop launcher discovers Visual Studio's bundled CMake for child processes; alternatively install CMake on PATH or set `CMAKE` to its executable.
3. Install Rust from [rustup.rs](https://rustup.rs/) with the MSVC toolchain, then reopen the terminal. `rust-toolchain.toml` selects stable / x86_64-pc-windows-msvc; the locally validated Rust version was 1.99.0.
4. Install or confirm the presence of [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).

See the official [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

```powershell
git clone https://github.com/Onlyaiyancc/SuperSearch.git
cd SuperSearch
npm ci
npm test
npm run check
npm run desktop:test
npm run desktop:check
npm run desktop:smoke:bundled
npm run desktop:dev
```

You do not need the maintainer's `.tools/`. `scripts/desktop.mjs` prefers an existing project-local Rust installation, otherwise it uses standard Rust while preserving its `CARGO_HOME` and `RUSTUP_HOME`. Only child-process environments change; the global PATH is not modified. Run Cargo operations through `npm run desktop:*`.

## Builds and checks

```powershell
npm run notices
npm run verify -- --smoke
npm run desktop:build
```

The installer is written to `src-tauri/target/release/bundle/nsis/`; the standalone executable is `src-tauri/target/release/supersearch.exe`. Exit any running app at the same path from its tray menu before replacing the executable.

Install the Rust audit tool with `npm run desktop:audit:install`, then run `npm run verify -- --smoke --audit-rust`. The first build downloads locked dependencies and packaging tools, so network access is required. Commit both dependency lockfiles.

## Debugging and privacy

`npm run dev` provides a browser preview with labeled synthetic data. It does not perform native searches or launch local apps. Native fixtures and reports live under `.tools/` and `test-results/` and are excluded from Git.

Normal operation sends no telemetry by default. Opened-item paths and usage counts may be stored locally; history can be disabled or cleared in settings. Explicit web searches pass queries to the default browser and Bing. Remove personal paths and device details before sharing diagnostics.

Physical IME, monitor interactions, installation/upgrade/uninstall and sleep/wake require their own native evidence. Short benchmarks and synthetic queries do not establish long-duration resource behavior.

## Dependency updates

Dependabot checks weekly. npm and Cargo patch updates are grouped into PRs; this does not automatically merge the PRs. Minor releases of 0.x dependencies may be incompatible and need individual review. GitHub Actions minor and patch updates are grouped; action references remain pinned to full commit SHAs. Version-update PR limits are 3, 3 and 2 respectively. Security updates remain enabled independently of those limits.

On a dependency branch, run `npm ci` and `npm run notices`, then review and commit changes to the lockfiles and `docs/licenses/THIRD-PARTY-LICENSES.txt`. The CI error `Dependency notices are stale` means notices do not match resolved dependencies; do not bypass the check or manually replace version numbers alone. For `Missing upstream license texts`, verify the relevant upstream version and update `docs/licenses/upstream/sources.json` and the original texts.

Before committing dependency changes, run `npm audit`, `npm run notices -- --check` and `npm run verify -- --smoke --audit-rust`. Tauri/native dependency changes also require a release build and isolated engine smoke test. CI uploads and downloads build artifacts to compare names and SHA-256 digests; that verification does not publish a release.

Do not mask frontend version conflicts with `--force` or `--legacy-peer-deps`. Assess Vite and the Svelte Vite plugin together, and keep TypeScript within svelte-check's peer range. See the [native acceptance checklist](testing/beta-native-acceptance.md) for physical checks.

## Documentation language

Use English for current public guides, release notes, issue templates and repository metadata. Keep [README.md](../README.md) and [README.zh-CN.md](../README.zh-CN.md) aligned. The application UI and source-comment language remain Chinese. Historical design and evidence records retain their original language and scope.
