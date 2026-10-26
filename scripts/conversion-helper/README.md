# Local PDF and OCR conversion components

`helper.py` is a single-request, offline worker used by the native SuperSearch conversion queue. The user-facing app supplies authorized staged files and installs the packaged worker as an optional component. Users do not need Python or pip.

## Protocol

Write one UTF-8 JSON object to stdin and close stdin:

```json
{
  "source": "C:\\staged-job\\document.pdf",
  "outputDir": "C:\\staged-job\\output",
  "options": {
    "target": "docx",
    "quality": 90,
    "dpi": 150,
    "icoSize": 256,
    "pages": "1-3,5",
    "sheet": 1,
    "merge": false,
    "ocr": false
  }
}
```

Success returns `{"outputs":["C:\\staged-job\\output\\converted.docx"]}` and exit code 0. Failure returns `{"outputs":[],"error":"Conversion ..."}` and exit code 1. Errors contain no document body or stack trace. `--version` and `--probe` return the helper version without reading stdin.

- PDF → TXT/DOCX extracts editable text and reconstructs paragraphs; it does not restore page layout. Empty or scanned documents require explicit OCR instead of reporting an empty success.
- PDF → XLSX/CSV extracts tables. A PDF without detected tables returns an error. XLSX cell values are explicitly strings; CSV formula prefixes are escaped.
- PDF → PNG/JPG exports selected pages. PDF → PPTX puts one complete page image onto each slide; mixed page sizes are fitted without stretching.
- Image → PDF uses a white background and EXIF orientation. To merge images in order, add top-level `sources` and set `merge:true`. Animated/multipage sources require an explicit `pages` selection.
- OCR supports image/PDF → TXT/DOCX/searchable PDF. Add top-level `ocrExecutable` and `ocrData` paths to an installed Tesseract component and set `ocr:true`. Searchable PDF is rebuilt from page images and recognized text, so original annotations, forms, signatures, attachments and bookmarks are not retained.

The worker accepts at most 128 KiB of request JSON, 200 MiB per input, 50 million pixels per page/image, 500 pages, 100 image merge sources, 32 MiB extracted text and 2 GiB of outputs/workspace. Images and intermediates are processed page by page; final writes cannot overwrite existing files. The native host additionally enforces memory, time, workspace and cancellation limits on the complete process tree.

## Build and verify on Windows

Use an isolated environment inside the project:

```powershell
python -m venv --copies test-results/conversion-helper-venv
test-results/conversion-helper-venv/Scripts/python.exe -m pip install -r scripts/conversion-helper/requirements.lock
test-results/conversion-helper-venv/Scripts/python.exe scripts/conversion-helper/test_helper.py
test-results/conversion-helper-venv/Scripts/python.exe scripts/conversion-helper/test_package.py
test-results/conversion-helper-venv/Scripts/python.exe scripts/conversion-helper/package.py
python scripts/conversion-helper/public-ocr-build.py
test-results/conversion-helper-venv/Scripts/python.exe scripts/conversion-helper/public-ocr-build-verify.py
```

`package.py` uses PyInstaller onedir, fixes archive timestamps, includes the helper source, dependency lock, project license, CPython license and dependency/native PDFium notices, and writes a ZIP plus exact size/SHA-256 metadata to `artifacts/releases/Format-Conversion-2026-10-21/`. Rebuilding with another Python version/platform can produce different binaries; update the native trusted manifest only from the actual tested archive. All required notices must be present or packaging fails.

`public-ocr-build.py` builds Tesseract 5.5.3 from the four official source archives pinned in `public-ocr-build-sources.json`, using Visual Studio 2022, static Leptonica/libpng/zlib, and static CRT. Python 3.12+ is required for source archive safety filtering. It preserves the DLL name and C API used by screen OCR, includes the official `chi_sim`/`eng` fast models, all upstream licenses and provenance, and creates component 1.0.1 plus a matching source materials ZIP under `artifacts/releases/Public-OCR-2026-10-25/`. `package_ocr.py` forwards to this public source build. The old extracted 1.0.0 artifact remains local only. See [OCR-NOTICES.md](OCR-NOTICES.md).

To run actual English and Chinese OCR tests, point to the public source build:

```powershell
$env:SUPERSEARCH_TEST_TESSERACT = (Resolve-Path test-results/public-ocr-build/stage/tesseract.exe).Path
$env:SUPERSEARCH_TEST_TESSDATA = (Resolve-Path test-results/public-ocr-build/stage/tessdata).Path
test-results/conversion-helper-venv/Scripts/python.exe scripts/conversion-helper/test_helper.py
```

The two OCR tests are explicitly skipped when those paths are absent; a skipped test is not evidence of successful recognition. Fixtures are synthetic and generated in temporary directories, and outputs are reopened by independent PDF, Word, Excel, PowerPoint and image readers.
