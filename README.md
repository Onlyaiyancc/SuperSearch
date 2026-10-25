# SuperSearch

**English** | [简体中文](README.zh-CN.md)

A Windows app launcher and file-name search tool, built with Tauri 2, Rust, Svelte 5 and the Everything SDK. Find apps, files and folders with name, pinyin, initials and alias matching from a keyboard-driven search bar.

**[Download v0.1.8](https://github.com/Onlyaiyancc/SuperSearch/releases/tag/v0.1.8)** · [All releases](https://github.com/Onlyaiyancc/SuperSearch/releases) · [Report a bug](https://github.com/Onlyaiyancc/SuperSearch/issues/new?template=bug.yml)

Windows x64 only. The application interface is currently Simplified Chinese. Release binaries are **unsigned** and require Microsoft WebView2 Runtime.

![Search interface with synthetic sample results](docs/images/search-preview.png)

## Features

- Search installed apps, files and folders by name; use pinyin, initials and aliases for app matching.
- Open the search bar with a configurable global shortcut and navigate results with the keyboard.
- Prioritize matching items you have opened before; pin items and configure search locations.
- Use a dedicated Everything service for fast NTFS indexing, with directory scanning when the service is unavailable or the file system is unsupported.
- Choose light, dark or system appearance, and recall the window at its remembered position or centered on the monitor containing the pointer.
- Search the web explicitly, or use Bing as a fallback when a completed local search has no matches.

## Download and install

Choose an asset from the [latest stable release](https://github.com/Onlyaiyancc/SuperSearch/releases/latest):

| Asset | Use |
| --- | --- |
| `SuperSearch_0.1.8_x64-setup.exe` | Recommended for the complete installation and dedicated indexing service. Requires administrator permission. |
| `SuperSearch-portable-x64.zip` | Extract the complete archive and run `SuperSearch.exe`. Keep all bundled license files. WebView2 must already be installed. |
| `SuperSearch-full-x64.zip` | Recommended for screen URL recognition: includes the private PDF/OCR components. Pandoc and Office install on demand from their official sources. |
| `SHA256SUMS.txt` | SHA-256 checksums for the release attachments. |
| `Release-Acceptance.md` | Validation evidence and remaining limitations for this build. |

Before upgrading, exit the old instance from the system tray. The installer registers `Everything (SuperSearch)` separately from a personal Everything installation. If WebView2 is missing, it downloads the runtime; restore network access and retry if the download fails. Portable mode uses directory scanning when the dedicated service is unavailable. Initial indexing can take several minutes; the advanced settings panel refreshes the loading state automatically.

The app and installer are unsigned, so Windows may display an unknown-publisher or SmartScreen prompt. Download from this repository's Releases page and compare the downloaded file with `SHA256SUMS.txt`:

```powershell
Get-FileHash .\SuperSearch_0.1.8_x64-setup.exe -Algorithm SHA256
```

## Quick start

| Shortcut | Action |
| --- | --- |
| `Alt+Space` | Show or hide the search bar; configurable in settings. |
| `↑` / `↓` | Select a result. |
| `Space` | After enabling file preview, explicitly select a file with arrow keys and press Space; Esc returns to results. |
| `Enter` | Open the selected result, or search Bing after a completed local search returns no matches. |
| `Ctrl+Enter` | Reveal the selected local item in File Explorer. |
| `Shift+Enter` | Search the current query on Bing. |
| `Tab` | Cycle available path completions. |
| `Ctrl+,` | Open settings. |
| `Esc` | Close the current panel or hide the search bar. |

In **Settings → General**, change the shortcut, theme and display positioning. The positioning choices are “记住上次位置” (remember the last position) and “跟随鼠标所在显示器” (follow the pointer's monitor). Use a result's action menu to pin it or edit an application's aliases; configure search locations under **Settings → Search**.

![Settings interface with browser sample data](docs/images/settings-preview.png)

## Privacy and validation

**Screen QR scanning is opt-in.** Enable **Settings → Extensions → 屏幕扫码** and save, then enter exactly `扫码` / `scan` and press Enter (`qr` and `扫一扫` also work). SuperSearch briefly hides and recognizes visible URL text and QR codes from the pointer's monitor locally. Web links appear above QR results and each offers copy/open actions; nothing opens automatically. First use prepares the bundled OCR component, so retain the complete package's `components` folder. An optional dedicated shortcut can be configured in the extension card. No screenshots, OCR text or scan history are saved. See the [screen QR guide](docs/screen-qr.md) for limits and verification scope.

**Local format conversion is opt-in.** Enable **Settings → Extensions → 格式转换**, then enter `convert` / `转换` or use a file result's action menu. Basic image conversion is built in; document/PDF/OCR components are installed on demand. See the [conversion guide](docs/format-conversion.md) for formats, offline component import and output-quality limits.

The optional **Translation & dictionary** extension is disabled by default. Enable it in **Settings → Extensions** and save. Type `翻译 hello` / `translate hello` for English → Simplified Chinese, `翻译 开发计划` / `translate 开发计划` for Chinese → English, or `查词 hello` / `dictionary hello` for English phonetics, definitions and examples. The short aliases `fy` and `dict` still work. Separate the command and content with a space; English commands are case-insensitive. Press Enter or the submit button to request a result; press Enter again to copy it. The direction selector can override the initial Chinese/English choice.

This extension uses [MyMemory](https://mymemory.translated.net/doc/spec.php) and [Free Dictionary API](https://dictionaryapi.dev/) without API keys. Only explicit submissions send the entered text to the selected provider. Translation accepts at most 500 UTF-8 bytes per request; dictionary lookup accepts one English word. Public services have usage and availability limits. No persistent translation history is created. Dictionary definitions are English; use translation for Chinese meanings. Browser preview uses clearly marked synthetic replies and never contacts these services or writes to the system clipboard.

Normal operation sends no telemetry by default. Settings and usage history stay on the local machine; history can be disabled or cleared in settings. An explicit web search passes the query to the default browser and Bing.

**File preview and countdown reminders are also disabled by default.** Enable them separately under **Settings → Extensions** and save. Open previews with Space after explicit selection or from the result menu. Only regular local files are read; network locations and directory links are unsupported.

| Preview | Limits and behavior |
| --- | --- |
| PDF | 100 MiB; native Windows rendering with previous/next page controls, page jumps and zoom |
| DOCX / XLSX / PPTX | 50 MiB; text/table content, worksheet or slide selection; original layout is not fully reproduced |
| PNG / JPEG / GIF / WebP / BMP / TIFF / ICO | 50 MiB and 64 million decoded pixels; large images use bounded thumbnails, small images retain adaptive height |
| Text / Markdown / code / configuration | 1 GiB; approximately 1 MiB per segment; UTF-8, UTF-16 and GB18030 |

HTML and SVG show source only. Markdown never loads external resources. Office ZIP/XML/output limits may reject oversized expanded content or show an excerpt (up to 10 sheets, 200 rows and 40 columns per sheet, or 100 slides). External Office relationships are rejected; legacy DOC/XLS/PPT and password-protected documents require the associated application. Files are never uploaded for preview.

Images and PDFs support fit-to-window and 10%–400% zoom with scrolling. With the content region focused, `PageUp` / `PageDown` switches PDF pages or text segments; left/right arrows also turn PDF pages in fit mode. `Ctrl` + `+` / `-` adjusts zoom and `Ctrl+0` restores fit. Inputs and controls retain their native key behavior; `Esc` returns to search.

Type `提醒 20分钟 休息` / `remind 20m 休息`, `倒计时 30秒` / `timer 30s` or `提醒 1小时30分钟 开会` and press Enter to create a reminder; type `提醒` / `remind` to list or cancel. The aliases `reminder` and `countdown` also work. Commands require a space before a duration; English commands are case-insensitive. Durations accept s/m/h/d, second/minute/hour/day and Chinese units, case-insensitively, including combinations such as `1h30m` and `1d 2h`. The range is 1 second to 7 days, with at most 20 pending reminders stored locally. Hiding the window keeps timers running. A fully exited application cannot notify until restarted; startup resumes pending timers and groups overdue items. Saving a disabled reminder extension cancels pending reminders. Windows notifications use the SuperSearch application name and icon; allow system notifications; completed status remains in the list if notifications are suppressed. Browser preview uses session-only synthetic reminders and sends no system notifications.

See this version's [release notes](docs/releases/v0.1.8.md) and attached `Release-Acceptance.md` for the exact build, component, OCR and installation checks. Earlier v0.1.7 cloud installation results remain historical evidence for that binary.

Physical IME, shortcuts, sleep/wake, monitor hot-plug, mixed 150%/200% DPI and interactive UAC are not fully established by this version's automatic tests. Hosted Windows checks do not cover every Windows 10/11 configuration. Older 30-minute reports do not establish long-duration behavior for this release.

Screenshots and browser preview use synthetic data. They do not perform native searches or launch local apps.

## Build from source

Prepare Windows x64, Git, Node.js 24 LTS, Rust stable MSVC, Visual Studio C++ Build Tools with the Windows SDK, and WebView2. See the [development guide](docs/development.md) for setup and troubleshooting.

```powershell
git clone https://github.com/Onlyaiyancc/SuperSearch.git
cd SuperSearch
npm ci
npm run desktop:dev
```

Validate and build:

```powershell
npm run verify -- --smoke
npm run notices -- --check
npm run desktop:build
```

Run Rust commands through `npm run desktop:*`. The wrapper uses an existing project-local toolchain or a standard Rust installation; copying the maintainer's `.tools/` is unnecessary. `npm run dev` starts a browser preview with sample data. See the [release guide](docs/releasing.md) for packaging and publication.

## Project structure

| Directory | Contents |
| --- | --- |
| [src](src/) | Svelte / TypeScript frontend. |
| [src-tauri](src-tauri/) | Rust backend, installer, bundled engine and vendor notices. |
| [public](public/) | Static assets and icon source. |
| [scripts](scripts/) | Build, diagnostics, license and release tools. |
| [tests](tests/) | Frontend and tooling tests. |
| [docs](docs/README.md) | Development, release, design and validation documentation. |

Dependencies, build caches, personal configuration and local diagnostic artifacts are excluded from Git. Local deliveries belong under `artifacts/releases/`, with historical material under `artifacts/archive/`.

## Contributing and license

Bug reports and focused contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md); report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

Original SuperSearch code is [MIT licensed](LICENSE). Bundled Everything, its SDK and other dependencies retain their own terms; the project license does not replace those terms. See [third-party notices](THIRD-PARTY-NOTICES.md) and [dependency license texts](docs/licenses/THIRD-PARTY-LICENSES.txt). SuperSearch is independent of voidtools and is not endorsed by it.
