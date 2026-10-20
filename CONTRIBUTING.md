# Contributing

Thank you for helping improve SuperSearch. Public project documentation uses English, with an English/Chinese README. The application interface is Simplified Chinese, source comments should be in Chinese, and development script output should be in English.

## Getting started

Follow the [development guide](docs/development.md) to prepare Windows x64. Before filing an issue, search existing reports. Include the app version, Windows version, installer or portable mode, reproduction steps, expected behavior and actual result. Remove personal information from screenshots and diagnostics.

Discuss substantial features in an issue before implementation. Small fixes can go directly to a pull request; keep changes focused and add meaningful regression coverage for the affected behavior. Original contributions are provided under the project's MIT license. Preserve the original licenses of third-party content.

## Validation

```powershell
npm test
npm run check
npm run desktop:test
npm run desktop:check
```

For engine changes, also run `npm run desktop:smoke:bundled`. For UI changes, inspect light, dark and narrow layouts. After dependency changes, run `npm run notices` and commit the updated notices. Do not commit `.tools/`, `node_modules/`, build output, private test data, certificates or keys.

For documentation-only changes, check links, commands, factual claims and language consistency; rebuilding the app is unnecessary unless executable behavior or configuration also changes. Keep the English and Chinese READMEs aligned on features, downloads, requirements and validation limits.

Pull requests should explain the problem, resulting behavior, verification and remaining limitations. Report security issues privately as described in [SECURITY.md](SECURITY.md).
