# Public OCR component 1.0.1 provenance

SuperSearch builds this Windows x64 OCR runtime from the official source releases listed in `SOURCE-ARCHIVES.json`. It is not extracted from the UB Mannheim Windows installer. The upstream code is unmodified; a CMake target output-name override names the shared library `libtesseract-5.dll` for compatibility with SuperSearch. The CLI is linked against that same DLL.

| Included software | Version | License | Distributed notice |
| --- | --- | --- | --- |
| Tesseract (CLI, C API, configs, PDF font) | 5.5.3 | Apache-2.0 | `licenses/Tesseract-Apache-2.0.txt` |
| Official tessdata_fast English and Simplified Chinese models | 4.1.0 | Apache-2.0 | `licenses/tessdata-fast-Apache-2.0.txt` |
| Leptonica | 1.85.0 | BSD-2-Clause | `licenses/Leptonica-BSD-2-Clause.txt` |
| libpng | 1.6.50 | libpng-2.0 | `licenses/libpng-LICENSE.txt` |
| zlib | 1.3.1 | Zlib | `licenses/zlib-README-and-license.txt` |

Leptonica, libpng, zlib, and the MSVC runtime are linked statically. Only `tesseract.exe` and `libtesseract-5.dll` are distributed as native runtime files. Windows system DLLs are not redistributed. No GPL/LGPL native libraries, networking libraries, archive libraries, OpenMP runtime, training tools, or installer files from the earlier local package are included.

The application supplies PNG images (including rasterized PDF pages) or raw pixels to this runtime. This build supports PNG/BMP/PNM inputs and searchable PDF output; JPEG/TIFF/GIF/WebP/JP2 codecs are disabled. Existing OCR engine behavior and the official fast models are retained, including the legacy engine build option.

`PROVENANCE.json` records the exact official archive URLs, sizes, SHA-256 digests, model digests, compiler commands and native binary digests. The accompanying `supersearch-ocr-sources-1.0.1.zip` contains the four original source archives, the checked-in build and verification scripts, source manifest, notices, and licenses. Build with Python 3.12+ and Visual Studio 2022 C++/CMake tools using `python scripts/conversion-helper/public-ocr-build.py`; Python is a build tool and is not included in the component. Pillow and pefile are used only by the verification script.

Official upstream sources:

- Tesseract: https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3
- Leptonica: https://github.com/DanBloomberg/leptonica/releases/tag/1.85.0
- libpng: https://github.com/pnggroup/libpng/releases/tag/v1.6.50
- zlib: https://zlib.net/fossils/zlib-1.3.1.tar.gz
- Models: https://github.com/tesseract-ocr/tessdata_fast/tree/4.1.0

The earlier 1.0.0 archive was an unpublished local verification artifact extracted from a third-party installer. It is superseded by 1.0.1 and must not be used as the public release asset. Its original provenance remains in the historical local archive.
