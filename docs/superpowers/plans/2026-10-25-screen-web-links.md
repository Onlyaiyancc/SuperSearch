# Screen web links correction plan

- [x] Replace QR-content extraction UI with a screen URL group above QR results.
- [x] Recognize screen text and QR codes from the same captured frame.
- [x] Reuse the pinned OCR package with bounded isolated execution and cancellation.
- [x] Resolve copy/open targets from current native session IDs.
- [x] Verify shared-memory results, cancellation, limits and OCR warnings.
- [x] Run full checks and actual OCR synthetic-image fixture; record the separate desktop-capture limitation.
- [x] Build the final release and pair its OCR/engine smoke evidence.
- [x] Deliver a complete portable package with companion components and evidence.

The design is recorded in [screen web links design](../specs/2026-10-25-screen-web-links-design.md). This corrects the superseded QR-content extraction interpretation; its historical evidence is not acceptance evidence for screen text recognition.

## Verification

Frontend: 353 passing tests, clean Svelte/TypeScript checks and 18 production browser scenarios. Native: 281 passing tests, three existing opt-in probes ignored, clean compilation check. Actual Tesseract recognition and QR decoding of the unchanged synthetic image identified all three expected visible URLs and the QR code. Real DLL probes reproduced the extended-path initialization problem before its repair; the spatial OCR correction retains distinct URLs and ordinary word boundaries. Review's split-dot truncation case was reproduced before its repair and is covered by a passing regression.

The current desktop's real-screen QR fixture could not detect its displayed code with either the prior or new executable. Computer UI observation approval timed out. Real physical screen capture, mixed-DPI/multi-monitor behavior and physical shortcut interactions are therefore unverified by this delivery; image and synthetic-browser evidence must not be presented as physical acceptance.

Final release executable SHA-256: `b21b6f810e2276f557e8a23be59c827a26ee3195532b4b0e0646c5715fc4c10e`. The paired release executable passed the actual OCR image fixture (354 ms inside the diagnostic) and bundled-engine recovery diagnostic (598 ms). The complete ZIP contains the same verified executable and all three pinned component archives; ZIP executable identity and required entries were checked after compression. Delivery: `artifacts/releases/Screen-Web-Links-2026-10-25/`. No commit, publication or system Office installation was performed.
