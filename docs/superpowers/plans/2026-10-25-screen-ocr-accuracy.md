# Small screen text OCR accuracy

The reported screen address `https://tjch.hotjob.cn` was recognized as `https://tich.hotjob.cn`. The same error was reproduced with the previously delivered worker on fixed synthetic Segoe UI 12/16-pixel text. Parsing was not changing the character; Tesseract's small-glyph recognition produced the wrong hostname.

The worker now resamples its grayscale input with Lanczos before recognition. It prefers 3x enlargement, falls back to 2x or the original size to keep the processed image within 40 million pixels, and retains the existing isolated-process timeout and cancellation. QR decoding, URL parsing and copy/open targets remain unchanged. There is no hostname replacement table or `i`/`j` substitution.

The investigation follows Tesseract's [input quality and rescaling guidance](https://github.com/tesseract-ocr/tessdoc/blob/main/ImproveQuality.md). Synthetic screen text and fixed PNG regressions are used; the user's original screen image was not available.

Verification:

- Previously delivered worker: 5/12 targeted Latin hostname/control samples passed; the exact reported `j` to `i` error was reproduced.
- Final worker: 12/12 passed, including valid `tich` addresses that must retain `i`.
- Native suite: 283 passed, four opt-in tests ignored by default. The new actual-Tesseract PNG regression was also run explicitly and passed.
- Frontend: 353 passed; Svelte/TypeScript and native compilation checks passed. Browser assets remain identical to the earlier 18-case UI QA.
- Final release executable passed the original three-URL/QR image fixture and bundled-engine diagnostic.

The actual original screen still needs user retesting; OCR remains imperfect across arbitrary fonts and rendering. Delivery: `artifacts/releases/Screen-OCR-Accuracy-2026-10-25/`. Executable SHA-256: `efd00368b156e603ff94edfeca540e790beca67ef01117c92c9ae8aa8817d095`. No dependencies, user settings, commits or public releases were added.
