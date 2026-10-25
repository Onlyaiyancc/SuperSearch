# Translation and dictionary extension

The user selected translation/dictionary as the first optional extension and chose public services without API keys. Implement an extension settings page with one built-in extension, disabled by default. This version has no third-party installation mechanism.

## Interaction

- `fy <text>` / `翻译 <text>` opens translation; `dict <word>` / `查词 <word>` opens English dictionary lookup. Prefixes require a word boundary. Bare prefixes show instructions.
- Typing never sends network requests. Enter or the submit button explicitly sends the displayed text. Translation supports English → Simplified Chinese and Simplified Chinese → English, selected initially using the presence of Han characters, with a visible direction selector.
- Translation uses MyMemory's documented public GET endpoint, limited to 500 UTF-8 bytes per request. Dictionary uses Free Dictionary API's English endpoint and shows phonetics, parts of speech, English definitions and examples. It does not represent English definitions as Chinese dictionary entries.
- Results render as escaped text, include provider attribution, and support copying. No audio playback, clipboard monitoring, automatic paste or persistent lookup history.
- Disabled commands explain how to enable the extension. Settings changes follow the existing draft/save/cancel flow and survive restart; old settings default to disabled.
- Extension cards default to only an icon, title, subtitle and independent enable switch. The title area acts as a native disclosure without a separate text label or arrow, revealing examples, service/privacy information and limits on click or keyboard activation; revisiting the tab starts collapsed while preserving the settings draft.

## Boundaries

`src/translation.ts` owns prefix parsing and stale-response handling. `src/TranslationPanel.svelte` owns plugin UI. App integration intercepts recognized prefixes before local search or Bing fallback, resets the panel on query change/hide/settings navigation, and reuses the bounded settings window height.

`src-tauri/src/translation.rs` owns validation, fixed-host HTTPS requests, response limits and parsing. Commands recheck persisted enablement. Request timeout is 10 seconds, responses are capped at 256 KiB, redirects are disabled, and only one lookup runs at a time. No query text or provider response is logged. Network awaits hold neither store nor search-cache locks. Clipboard operations reuse the existing native clipboard writer.

## Verification

Test command boundaries, byte limits, invalid modes/directions, disabled defaults, persistence, response parsing, quota/errors, request serialization and stale responses. Run frontend tests/check/build and desktop tests/check. Use production browser UI with synthetic service replies for keyboard, disabled, loading, result, error, settings, dark/narrow and stale-response scenarios. Separately probe real services with synthetic text; report network availability honestly.
