# Screen web links correction

The requested extraction concerns URLs visibly printed on the screen, independently of QR content. One explicit scan captures the pointer's monitor once and recognizes both screen text and QR codes. Web links appear above QR results; each link offers copy and open. Nothing opens automatically. A screen with URLs and no QR codes still produces results.

Reuse the pinned local Tesseract component, automatically preparing the companion package on first use without enabling document conversion. OCR failure is nonfatal and leaves QR results available. Screenshots, OCR text and recognized links pass between native processes through bounded shared memory. The isolated worker retains the existing process job, timeout and cancellation boundaries. No screen data is sent to a recognition service.

The frontend submits native session/link IDs for actions. Close, rescan, disable and navigation invalidate results and cancel ongoing OCR. Keyboard selection follows visible order: web links, then QR codes. The previous per-QR extraction button is removed.

Recognize explicit HTTP/HTTPS, www addresses and common bare domain addresses. Missing schemes use HTTPS. Results are canonicalized, deduplicated and limited to 16 screen links; uncertain OCR characters are not silently corrected. Users inspect the recognized address before opening.

Use OCR word coordinates to repair narrowly separated URL fragments within the same text line. Preserve ordinary word gaps, separate links and line boundaries. Do not remove spaces from an entire screen or guess visually similar hostname characters. Normalize extended Windows paths only at the Tesseract data-file boundary; DLL loading retains the absolute UTF-16 path and restricted dependency search.

Verification includes parser/session/worker regression tests, full frontend/native checks, production browser layout and action scenarios, and actual OCR of a synthetic image with visible URL text and a separately encoded QR code. The real Windows screen fixture is separate and must report its own result. Synthetic image/UI checks do not establish physical screen capture, multi-monitor or mixed-DPI acceptance.
