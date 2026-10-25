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

The optional file preview uses Marked and DOMPurify for Markdown presentation, encoding_rs for text decoding, and zip/quick-xml for bounded Office content extraction. PDF rendering and TIFF/ICO/image thumbnails use Windows native APIs through the Windows bindings. Countdown notifications also use native WinRT APIs. Their package licenses are included in the generated notices; local user files and reminder content are not bundled software.

Regenerate those notices after dependency changes with `npm run notices`. This requires frontend dependencies and a working Rust toolchain. Build/development package notices are included for traceability; their presence does not mean all those tools are shipped in the app. Microsoft WebView2 is a separately provided Windows runtime governed by Microsoft's terms.

The release installer includes this file, the project license, and the generated dependency notices under its `licenses` directory. Preserve these files when redistributing the portable program.

## Screen QR decoder

The optional local screen QR extension uses the pinned `zxing-cpp` 0.5.3 Rust wrapper and its bundled ZXing-C++ 3.1.1 core under Apache-2.0. The wrapper's bundled build also contains libzint under BSD-3-Clause. Original license texts and upstream source references are preserved in the generated dependency notices and `docs/licenses/upstream/zxing-licenses.txt`. Images are processed locally; no external QR decoding service is used.

## Optional language services

The optional translation/dictionary extension contacts [MyMemory](https://mymemory.translated.net/doc/spec.php) or [Free Dictionary API](https://dictionaryapi.dev/) only on explicit submission after enablement. These hosted services and their content are not licensed by SuperSearch's MIT license. Dictionary results display provider attribution and available entry license/Wiktionary source information, and retain that attribution when copied. No dictionary corpus is bundled with the application. Service availability and usage limits are controlled by their operators.

## Optional local conversion components

Basic image conversion uses the Rust image library and its selected codecs; their original license texts are included in the generated dependency notices. The conversion extension may install or reuse separately running [LibreOffice](https://www.libreoffice.org/) and [Pandoc](https://pandoc.org/). These projects retain their own licenses and trademarks. They are not relicensed under SuperSearch's MIT license and are not included in the main application installer.

The separately distributed PDF and OCR component archives include a component manifest, dependency versions, upstream sources and original license texts. The PDF helper includes Python, PDFium and document/image libraries. The OCR archive includes Tesseract, its native dependencies and separately identified language data. Preserve each archive's `licenses` directory and provenance documents when redistributing; the main application's notices do not replace the component notices. Only explicitly installed local components process conversion files; the extension does not use an online conversion service.

## Artwork

The black/white SuperSearch application mark and project screenshots are included under the project's MIT license. Lucide/Feather icons retain the copyright and ISC/MIT terms included in the dependency notices. Product names in sample results identify examples and do not imply endorsement.
