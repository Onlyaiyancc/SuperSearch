# SuperSearch

SuperSearch is a lightweight omnibox for Windows built with .NET 8 and WPF. It focuses on fast keyboard workflows, keeping the UI minimal while providing rich search capabilities.

## Key Features

- **Global summon** �C press `Alt+Space` anywhere to pop up the search bar. `Shift+Enter` forces a Bing search, while `Esc` hides the window instantly.
- **Unified results** �C type an application name, a URL, or arbitrary text; SuperSearch surfaces local apps, direct URL launches, or Bing web results respectively.
- **Optimised keyboard navigation** �C arrow keys move through results; the list keeps the selection visible by auto-scrolling. Double-click or `Enter` launches the highlighted item.
- **Tray integration** �C SuperSearch runs quietly in the notification area with options to show, hide, or exit the app. The tray icon shares the same custom search glyph as the main window.
- **Dark theme** �C a cohesive dark UI with a slim custom scroll bar that keeps focus on results.
- **Zero-install usage stats** �C recently launched apps bubble to the top of the default view; statistics are stored locally under `%LOCALAPPDATA%`.

## Architecture Overview

```
SuperSearch
|-- App.xaml / App.xaml.cs          # Application bootstrap, DI container, graceful shutdown
|-- MainWindow.xaml (.cs)           # UI layout, input handling, global hotkey registration
|-- ViewModels/MainViewModel.cs     # Search orchestration, throttling, command execution
|-- Models/                         # ApplicationEntry, SearchResult, usage DTOs
|-- Services/                       # Indexing, fuzzy search, URL detection, process launching
|-- Utilities/                      # FuzzyMatcher, IconLoader, lightweight logging
|-- Interop/                        # Win32 hotkey helpers, Mica backdrop utilities
`-- Assets/app.ico                  # Shared icon for window and tray
```

The app uses [`Microsoft.Extensions.DependencyInjection`](https://learn.microsoft.com/dotnet/core/extensions/dependency-injection) for basic dependency wiring and [`CommunityToolkit.Mvvm`](https://learn.microsoft.com/windows/communitytoolkit/mvvm/introduction) to keep view-models concise.

## Building

```powershell
# from repository root
cd SuperSearch

dotnet build
```

## Publishing (Windows x64)

```powershell
cd SuperSearch

dotnet publish SuperSearch.csproj `
  -c Release `
  -r win-x64 `
  --self-contained false `
  /p:PublishSingleFile=true
```

Published binaries (including the custom icon) are placed under `SuperSearch\bin\Release\net8.0-windows\win-x64\publish`.

## Keyboard Shortcuts

| Shortcut         | Action                                  |
|------------------|-----------------------------------------|
| `Alt+Space`      | Show/hide SuperSearch                   |
| `Shift+Enter`    | Force a Bing search for the current text |
| `Enter`          | Launch highlighted entry                 |
| `Esc`            | Hide the window                         |
| `��` / `��`        | Navigate result list                    |
| `Ctrl+Space`/`Ctrl+Shift+Space` | Automatic fallback hotkeys if `Alt+Space` is unavailable |

## Configuration Notes

- Local usage telemetry is stored in `%LOCALAPPDATA%\SuperSearch\usage.json`; delete the file to reset launch statistics.
- If the tray icon displays the default shell image, close SuperSearch, clear Windows icon cache, and restart the app (the icon is packaged at `Assets\app.ico`).
- Hotkey registration failures are surfaced via toast dialogs; choose an alternative shortcut from the tray menu if needed.

## Roadmap Ideas

- Plugin model for additional data sources (recent documents, browser history, etc.).
- Theme toggles (light/auto) and accent colour customisation.
- More powerful web integrations (Wikipedia, developer docs, custom search engines).

Feel free to fork and adapt SuperSearch to match your own workflows.
