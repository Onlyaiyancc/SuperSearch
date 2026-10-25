# Native candidate acceptance checklist

Use this checklist for Windows x64 candidate binaries. Record the source SHA, installer/portable SHA-256, Windows and WebView2 versions, display scaling, steps and actual results. Mark unexecuted checks **unverified**. Save screenshots and reports to `test-results/`, remove personal paths, then archive them under the matching `artifacts/releases/` version directory.

Automated tests, isolated smoke diagnostics and artifact checks do not replace physical interaction checks. Run installation lifecycle tests on a disposable hosted runner or a restorable Windows VM, never the maintainer's everyday machine.

| Scenario | Steps and pass criteria | Evidence limits |
| --- | --- | --- |
| First-index status | Open advanced settings immediately after a fresh start. Loading must become ready without reopening settings. Repeat app exit, launch and settings entry. | Regression tests cover status refresh; real NTFS timing needs native evidence. |
| Chinese IME | Use Microsoft Pinyin, choose a candidate with Space, confirm with Enter, then press Enter to launch. Composition must not launch an app accidentally. | State tests are not physical IME evidence. |
| Shortcut and single instance | Repeat Alt+Space, hide/recall and duplicate launches. One main instance remains, focus returns to search, and tray exit permits relaunch. | Requires real focus and shortcut checks. |
| Monitors and scaling | Move between 100%, 150% and 200% monitors, move the window and disconnect a secondary display. Search/settings must remain within the work area. Test remembered and pointer-monitor positioning. | Synthetic geometry tests do not establish physical hot-plug behavior. |
| Sleep/wake | Search, sleep, wake, query again and open settings. Interaction and provider status recover without a permanently stuck query. | Requires real system lifecycle checks. |
| Fresh install | In a disposable environment, install and verify the dedicated service, Start menu entry, search, exit and WebView2 behavior. Separately accept an interactive UAC prompt. | Hosted lifecycle checks use elevated silent installation and do not exercise UAC clicks. |
| Upgrade | Install the baseline, configure shortcuts/aliases/pins, exit from the tray and install the candidate. Verify retained data and service availability. | Record both versions and hashes; a same-version replacement is not upgrade evidence. |
| Uninstall and coexistence | Install personal Everything too. Uninstall SuperSearch and verify its program/service removal, personal Everything functionality and user-data retention behavior. | Use an isolated environment. |
| Portable mode | Extract the complete ZIP without a dedicated service. Use a synthetic folder and verify scanning, searching, opening fixtures and exit. | Engine smoke covers directory queries; the complete UI flow needs its own evidence. |
| 30-minute resources | Run `npm run desktop:soak` against the exact candidate. Record memory change/trend, query latency and failures; retain raw samples. | Synthetic windows do not prove physical interaction or absence of memory leaks. |

Before a new beta, cover first-index status, IME, shortcuts/single instance, fresh install and upgrade. Disclose remaining gaps in release notes. Do not present older-build results as acceptance of a new build.

Stable candidates must demonstrate upgrades across different internal versions. The v0.1.8 baseline is the public v0.1.7 installer and its known hash. Installer, portable executable and installed executable versions must match the candidate source. Cold-index reports retain `initialIndex` observations and failures; resource reports identify the candidate executable hash. The maintainer decides whether the remaining gaps are acceptable before publication; preparation does not establish all-scenario acceptance.

## Automated installation lifecycle

Manually dispatch **Windows checks and pre-release** with `native_acceptance` enabled, the successful main build's `candidate_run` and full `candidate_sha`, and an empty tag. It reuses the exact build assets on a fresh GitHub-hosted Windows runner to install the baseline, upgrade, verify settings/aliases/pins/history, query NTFS, uninstall with personal Everything present, and install with fresh user data. Reports are uploaded as `native-installation-evidence`.

`scripts/native-installation.ps1` requires PowerShell 7 and a GitHub-hosted Windows administrator runner. It rejects existing relevant installations or services. Never run it on an everyday computer or spoof runner environment variables. The disposable runner does not operate on the maintainer's services or data.

This executes real installers and services using elevated silent installation. Physical UAC, IME and monitor interaction remain separate checks. Missing-WebView2 recovery is tested by the additional protocol below.

## Missing-WebView2 cloud protocol

After lifecycle checks, `native-installation.ps1` invokes `webview2-acceptance.ps1`. It permits only GitHub-hosted Windows administrator runners and rejects ordinary computers, self-hosted runners and other systems first.

The protocol verifies the Microsoft-signed runtime uninstaller and removes the real runtime, confirming both registry entries and runtime executables are absent. An outbound firewall rule limited to this candidate installer blocks runtime download. Installation must fail without leaving the program or dedicated service. After removing the rule, a retry must restore the runtime, match the expected application payload, start the service and pass a real native WebView frontend diagnostic. The candidate is then uninstalled.

Reports `webview2.json` and `webview2-ui.json` accompany the main report and exact candidate provenance in `native-installation-evidence`. The firewall rule is removed in `finally`. The protocol does not disable machine-wide networking, alter the maintainer's system or treat hidden registry entries as actual runtime removal.

This establishes hosted Windows silent-install behavior, not every Windows 10/11 version, interactive error wording or UAC clicks. Record a result only after a successful actual run.

## v0.1.7 published evidence

The [build](https://github.com/Onlyaiyancc/SuperSearch/actions/runs/39364003496) and [cloud acceptance](https://github.com/Onlyaiyancc/SuperSearch/actions/runs/39371289628) identify the exact published candidate. The release's `Release-Acceptance.md` distinguishes cloud checks, short native diagnostics, maintainer-reported interaction and remaining gaps. It is the summary for that release; this checklist defines procedures for future candidates.
