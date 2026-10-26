# Screen QR scanning

Enable **Settings → Extensions → 屏幕扫码** and save. The extension is disabled by default and has no default global shortcut.

Enter exactly `扫码` / `scan` and press Enter, or click **扫描当前屏幕**. The aliases `qr` and `扫一扫` also work; English commands are case-insensitive. Ordinary searches such as `qr.png`, `scanner.md` or `scan text` remain searches. Typing alone does not capture the screen. The selected monitor is the monitor containing the pointer at the start of each scan. The whole monitor is captured, including its taskbar area; SuperSearch temporarily hides so its own window does not cover a code.

Each scan recognizes HTTP/HTTPS URLs visibly printed on the screen and decodes QR codes from the same captured frame. **网页链接** appears first, above **二维码**. Screen URLs are independent of QR content: a screen containing readable URLs and no QR codes can still produce link results. Each webpage result shows its domain and URL, with **复制链接** and **打开链接** buttons. No link opens automatically.

Recognition also accepts `www` addresses and common bare domain names, using HTTPS by default when the screen text omits a scheme. Inspect the recognized domain and full URL before opening it.

QR results retain their original decoded content and **复制** action. A QR code whose entire content is an HTTP/HTTPS URL additionally offers **打开链接**. Other protocols and local paths are displayed as text. Up/Down selects screen URLs first, followed by QR results; Enter copies the selected screen URL or QR content. Tab reaches explicit action buttons. Escape closes the panel and clears its session. **重新扫描** captures the pointer's current monitor again. Action failures retain results for retry.

Screen text recognition uses the local OCR component and automatically prepares it on first use, independently of the format-conversion extension. Use the complete package and keep its `components` folder. Missing components, busy recognition and timeouts show a clear nonfatal warning; available QR results remain usable. No separate installation toggle is required. OCR can misread text, so inspect the recognized URL before opening it.

An optional dedicated shortcut can be recorded in the extension card's expanded details. It must differ from the main search shortcut. Saving verifies registration; conflicts preserve prior settings and bindings. Disabling the extension releases its active shortcut while keeping the configured chord available for later re-enablement.

## Processing and limits

- Capture, screen-text recognition and QR decoding happen in native code on this machine, only after an explicit action. No screen polling or online recognition service is used. URL recognition does not visit links or follow redirects.
- The original screenshot is not sent to the WebView, included in diagnostics or logged. The frontend receives recognized URLs, decoded QR text and small thumbnails sampled from inside QR quadrilaterals. Closing or replacing a session releases its results.
- A scan accepts at most 40 million pixels (160,000,000 raw RGBA bytes). Screens above that limit receive an explanation before pixel allocation. The full capture is not downscaled before decoding.
- OCR enlarges its in-memory grayscale input by up to three times to preserve small glyph details such as `j` versus `i`. The processed image still fits within 40 million pixels; large screens use a smaller factor. QR decoding continues to use the original capture. Recognition can still make mistakes; hostname characters are not guessed or substituted.
- At most 32 QR results and 16 distinct screen webpage links are retained per scan. Each QR result contains at most 16 KiB of text. The panel reports when either group reaches its count/content limit. QR thumbnails are at most 160 × 160 pixels. Uncopyable control-containing content is rejected instead of silently changing the copied value.
- Screen webpage links use HTTP/HTTPS; other protocols and local paths are excluded. Screen-link copy/open requests contain only native session and link IDs; QR actions use session and result IDs. The frontend does not submit arbitrary URLs. Rescanning or closing the session clears both groups.
- Protected desktops and capture-restricted windows may be inaccessible. A visually displayed code is not guaranteed to decode if it is too small, blurred, obscured or unsupported.
- Leaving the scan, recalling ordinary search, opening settings, hiding the app or disabling the extension invalidates old results. A late reply cannot replace the new view or reopen a canceled window. Passive application-list refreshes preserve active scans and displayed results.

The browser preview displays clearly marked synthetic replies and never captures the screen, writes the clipboard or opens a URL.

## Diagnostics

Run the built executable with `--smoke-test-qr` to briefly show a non-focusable synthetic QR window and exercise the real Windows capture/decoder pipeline. It uses no user store and saves no screen image. Its JSON result reports only dimensions, duration and whether the synthetic code was found. This check does not establish physical multi-monitor, mixed-DPI or full application interaction acceptance.

Run the built executable with `--smoke-test-screen-links` to show a synthetic frame containing visible plain-text URLs and a QR code together. This diagnostic exercises real screen capture and local OCR as well as QR decoding, checking that screen-text URL recognition works independently of the QR content. It does not save screenshots or recognized OCR text; its report contains diagnostic status rather than the captured image or full recognized text. It does not establish physical multi-monitor, mixed-DPI or full application interaction acceptance.

`--smoke-test-screen-links-image` instead draws a known fixture entirely in memory and tests the actual local OCR worker, URL extraction and QR decoder without reading the desktop. It may save the known synthetic fixture and its expected URLs under `test-results/screen-links/`; its report explicitly marks real screen capture as unverified.

After preparing the verified local OCR component, run `node scripts/desktop.mjs test real_small_text_keeps -- --ignored --nocapture` for the real-engine small-font `j`/`i` regression. It uses fixed synthetic PNG fixtures, with no screen capture or network access.

Build and test through the project desktop wrapper; do not run Cargo directly. See [development setup](development.md) for the QR decoder's CMake prerequisite.
