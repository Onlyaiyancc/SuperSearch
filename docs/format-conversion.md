# Local image and document conversion

Enable **Settings → Extensions → 格式转换** and save. Enter `转换` or `convert` to choose local files, or use **格式转换** from a file result's action menu. Ordinary filenames beginning with these words continue to use file search.

Choose a target and output directory, adjust any visible options, then select **开始转换**. Files are converted locally. Originals are retained; output name collisions receive a suffix instead of overwriting another file. The queue supports up to five batches of 100 files, processed serially. Hiding the search window leaves tasks running. The panel shows per-file results and cancellation; exiting with pending work requires confirmation.

Retries retain the original task's output directory and options, even after changing the current draft. A normal failed item can be retried after its batch finishes. Failed merged-image PDFs use **重试合并任务**, preserving all original images in order. To change the destination or options, create a new task instead. A replaced source or destination directory is rejected; select files and a valid output location again. CSV retry reuses the original reviewed delimiter/encoding only for the unchanged granted source.

## Supported families

| Input | Output |
| --- | --- |
| PNG, JPG/JPEG, WebP, BMP, TIFF, GIF, ICO | Common static image formats; PDF; text/Word through OCR |
| DOC, DOCX, ODT, RTF | Word-family formats, PDF, TXT, HTML; DOCX can export Markdown |
| XLS, XLSX, ODS | Spreadsheet formats, selected-sheet CSV, PDF |
| CSV | XLSX, ODS |
| PPT, PPTX, ODP | Presentation formats, PDF |
| Markdown, TXT, local HTML | Markdown, TXT, HTML, DOCX, PDF |
| PDF | Per-page PNG/JPG; editable text in TXT/DOCX; detected tables in XLSX/CSV; picture-slide PPTX; searchable PDF through OCR |

The target selector lists only directions shared by all selected files. Reorder images before merging into a PDF. Multi-frame or multi-page images require explicit frame/page selection instead of silently using the first frame. Ranges use forms such as `1`, `1-3` or `1-3,5`.

Adding a document to an image merge automatically clears the incompatible merge option. Compatible choices and an empty selection's draft are retained. Status updates are event-driven: task events refresh tasks, component events refresh components, and overlapping requests share one active read plus one pending follow-up. Disabling conversion stops new work and component queries while keeping existing tasks visible.

CSV imports let you choose comma, semicolon or tab separation and UTF-8, GB18030/GBK, UTF-16 LE or UTF-16 BE encoding. Preview each selected CSV before starting; the preview is bounded to 20 rows and 20 columns. CSV inputs are limited to 32 MiB, 100,000 rows and 1,024 columns and are imported as literal cell text instead of executable formulas.

PDF-to-Word reconstructs editable paragraphs, not the original page layout. PDF-to-PowerPoint puts a complete page image on each slide; individual page elements are not editable. Table extraction does not restore formulas and reports an error when no table is found. Scanned pages need OCR; recognition can contain errors. Searchable PDF is rebuilt from page images, so original signatures, forms, bookmarks and attachments are not preserved. Office conversions can change fonts, layout and animations. CSV exports one explicitly selected sheet and drops other sheets, styles and formulas.

## Optional components

Basic static image conversion is built in. The panel identifies any extra component needed:

- **Office 文档**: detects a supported system LibreOffice installation or downloads a pinned official installer. Complete the normal Windows installation, then refresh component status. SuperSearch does not silently install it or remove an existing system Office installation.
- **Markdown 与网页文档**: installs a pinned official Pandoc ZIP privately. PDF export also uses the Office component.
- **PDF 与可编辑文档**: select **安装组件** to install the matching companion PDF ZIP, including its runtime. Users do not need Python or pip.
- **扫描件文字识别**: select **安装组件** for the companion OCR ZIP with English and Simplified Chinese language data. OCR also requires the PDF helper.

Keep the full portable distribution together. **安装组件** first checks only these fixed files in `components/` beside the running executable: `supersearch-pdf-component-1.0.0-windows-x64.zip`, `supersearch-ocr-component-1.0.1-windows-x64.zip`, and `pandoc-3.12.1-windows-x86_64.zip`. It never searches another folder. Companion packages install without a network request. An absent companion falls back only to a published, pinned HTTPS download; PDF/OCR packages are also available from the pinned v0.1.8 release download. Linked/reparse files and directories are rejected.

Select **导入本地组件包** as an optional fallback when the matching archive is stored elsewhere. Both install paths validate exact size and SHA-256 against the trusted manifest included with the application; arbitrary executables or another version are rejected. Downloads and installation start only after an explicit button click. Hashing, extraction and capability probing run on a blocking worker. A cancelled or failed installation leaves the previous ready version intact. Download, checksum, invalid archive and missing dependency errors show distinct recovery instructions. Private components can be removed when no conversion uses them; system LibreOffice is managed through Windows.

Project-built packages are generated under `artifacts/releases/Format-Conversion-2026-10-21/`. Build scripts, protocol, dependency lock and local OCR provenance are documented in [scripts/conversion-helper](../scripts/conversion-helper/README.md). Local verification archives are separate from a public component release. Public redistribution of a repackaged OCR runtime requires the dependency materials identified in its provenance notices.

## Limits and privacy

Only authorized local regular files and directories are accepted. Network locations, device paths and directory links are rejected. The native process stages bounded copies before invoking an engine; the frontend cannot submit arbitrary source paths. Remote document resources, macros and executable conversion filters are not supported.

Limits include 200 MiB per source, 50 million pixels per image/page, 500 pages and 2 GiB per task workspace/output. External engines run in owned Windows process groups with memory and time limits. A failed/cancelled item preserves outputs already completed by other items. Search, translation, reminders and preview retain their separate settings and lifecycle.

Conversion files are not uploaded to a hosted conversion service. Component downloads fetch software only. No persistent conversion-content history is written; current-session results are bounded in memory. Browser preview displays explicitly marked synthetic samples and cannot perform conversion, download a component or write local outputs.
