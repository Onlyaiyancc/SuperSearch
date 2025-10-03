# Super Search Architecture (Draft)

## Overview
Super Search is a .NET 8 WPF desktop application that emulates a floating omnibox. The UI launches via a global hotkey and offers a fuzzy search experience across local applications, URLs, and Bing suggestions.

## Layers
- **UI (WPF)**: A single main window with acrylic background, list view of results, keyboard navigation, and visual states.
- **ViewModels (MVVM)**: `MainViewModel` orchestrates search input, result list, and action execution; `SearchResultViewModel` wraps result metadata.
- **Services**:
  - `ILocalAppIndexer`: Scans Start Menu shortcuts, registry uninstall entries, and known installation folders to produce `ApplicationEntry` records.
  - `ISearchService`: Performs fuzzy ranking across indexed applications and intent items (URL open, Bing search).
  - `IUrlDetector`: Validates and normalizes URLs.
  - `IBingSearchLauncher`: Builds Bing query URL and opens browser.
  - `IProcessLauncher`: Wraps `Process.Start` with ShellExecute, supports run-as-admin and auxiliary actions.
- **Models**: Plain DTOs for application entries, search results, and scoring metadata.
- **Infrastructure**: Hotkey registration, icon extraction cache, persistence of usage statistics (JSON file under `%LocalAppData%`).

## Data Flow
1. App start loads cached index from `%LocalAppData%` if present; otherwise triggers asynchronous rebuild.
2. User opens window (global hotkey) and types query.
3. `MainViewModel` throttles input updates, requests matches from `ISearchService`.
4. Results show intents in priority order:
   - URL intent if detector matches.
   - Local application results scored by fuzzy engine + usage weighting.
   - Bing fallback item when no confident local results exist.
5. User confirms; `IProcessLauncher` either launches app, opens browser to URL, or Bing search URL. Usage stats update for weighting.

## Technology Notes
- Target framework: `net8.0-windows` (`UseWPF`).
- JSON persistence via `System.Text.Json`.
- Fuzzy scoring implemented in-house (tokenized prefix + subsequence scoring) to avoid heavy dependencies.
- Acrylic effect done by extending `Window` with Windows 11 API (`DwmSetWindowAttribute` for Mica/Acrylic).
- Global hotkey via `RegisterHotKey` P/Invoke.

## Future Enhancements
- Shell context menu (run as admin, open file location).
- Pinyin matching.
- Plugin system for custom data sources.
