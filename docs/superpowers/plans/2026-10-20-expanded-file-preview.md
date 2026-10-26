# Expanded File Preview Implementation Plan

**Goal:** Add local PDF and Office content previews, larger images and paged text without changing normal search behavior.

**Architecture:** Keep file authorization in service.rs. Independent native modules own PDF rendering, Office extraction and bounded file/text/image handling; a preview session ID prevents stale work from taking ownership of current state. Separate Svelte content views render only bounded results.

## Tasks

- [x] Native PDF module `src-tauri/src/pdf_preview.rs`: test first with synthetic PDF, implement Windows renderer/cache/cancellation, return one bounded PNG page with page count.
- [x] Office module `src-tauri/src/office_preview.rs`: test first with synthetic DOCX/XLSX/PPTX ZIPs, reject bad archives/DTD/external relationships, cap extraction and return escaped text/table structures.
- [x] Core module `src-tauri/src/file_preview.rs` plus image helper: test larger limits and more formats, add bounded text segments and encoding boundaries, expose a validated bounded file opener for PDF.
- [x] Integrate new native modules and commands in main.rs/service.rs; never retain store/cache locks while reading or rendering. Invalidate preview sessions on hide/query change.
- [x] Frontend: extend result types, add PDF/text pagination and Office sections/sheet selection, include a unique preview ID and explicit close cleanup. Keep adaptive image height and existing compact extension cards.
- [x] Verify helper tests, native file fixtures, production UI lifecycle/layout, full frontend/Rust checks, dependency notices and local desktop build. Record evidence in test-results/expanded-preview and update user docs.
