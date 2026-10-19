# Third-party software

SuperSearch's original source code is licensed under [MIT](LICENSE). Third-party software retains its own copyright, license and trademark notices. The application license does not replace those terms.

## Everything and its SDK

- Everything 1.4.1.1032 x64 is an unmodified, separately running executable by David Carpenter / voidtools. SuperSearch uses it as a private indexing backend. SuperSearch is an independent project and is not endorsed by voidtools.
- Original executable provenance, hash and download location: [bundled engine README](src-tauri/resources/everything/README.md).
- The full vendor notice, including the bundled PCRE notice, is preserved in [License.txt](src-tauri/resources/everything/License.txt). It is distributed alongside the extracted engine and included in the installer resources.
- The SDK is an IPC wrapper, statically linked from the C sources under [src-tauri/vendor](src-tauri/vendor/README.md). Original MIT notices are retained in the source/header files; SuperSearch's bridge changes are described in the provenance README.
- The indexing executable's source is not maintained in this repository. Opening SuperSearch's source does not make a claim that every bundled executable has its source published here.

## Frontend and Rust dependencies

Tauri, Svelte, Lucide, the Windows bindings, SQLite/rusqlite and other dependencies are resolved by `package-lock.json` and `src-tauri/Cargo.lock`. The generated [dependency notices](docs/licenses/THIRD-PARTY-LICENSES.txt) retain available license and copyright texts from the installed packages and Windows Rust dependency graph, with package names and versions.

Regenerate those notices after dependency changes with `npm run notices`. This requires frontend dependencies and a working Rust toolchain. Build/development package notices are included for traceability; their presence does not mean all those tools are shipped in the app. Microsoft WebView2 is a separately provided Windows runtime governed by Microsoft's terms.

The release installer includes this file, the project license, and the generated dependency notices under its `licenses` directory. Preserve these files when redistributing the portable program.

## Artwork

The black/white SuperSearch application mark and project screenshots are included under the project's MIT license. Lucide/Feather icons retain the copyright and ISC/MIT terms included in the dependency notices. Product names in sample results identify examples and do not imply endorsement.
