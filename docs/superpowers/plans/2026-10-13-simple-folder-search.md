# Simple folder search

**Goal:** Implement the user's chosen direct input `folder path filename`, preserving the stronger border already delivered.

**Design:** A leading local drive path becomes the folder boundary and the remaining text stays a filename query. For `all` scope, a direct folder path selects files; explicit files/folders still work. Existing in/ext/type syntax remains compatible. Quoted space-containing paths are deterministic; unquoted paths with spaces use the longest existing local directory prefix, with bounded local filesystem checks. Invalid/control/device/UNC/parent traversal inputs remain rejected. A path alone should list indexed files in that directory (and subfolders).

**Interaction:** Add a concise example to the search placeholder/accessible description and settings help. Folder searches with zero results stay local on ordinary Enter; Shift+Enter retains explicit web behavior. Browser preview must either apply real scope/path semantics to clearly marked fixture paths or clearly report unsupported syntax, never imply native file connectivity.

**User refinement:** Also support `folder/filename`, including indexed folder names such as `项目/报告` and full paths such as `D:/项目/报告`. Named folders match all same-name path components with descendants, bounded by separators; full paths are anchored. Existing absolute directories remain directory queries; a trailing separator explicitly lists files. Ranking and historical candidates must use the same boundary rule as the native index.

- [x] Add failing parser tests for simple input, quoted paths, scope, drive root, bad paths/conflicting filters, and space-containing directories.
- [x] Implement minimal prefix parsing through existing local_directory validation and query compiler.
- [x] Add frontend scope feedback and prevent automatic web fallback for local path searches; cover with meaningful tests.
- [x] Extend native directory-boundary diagnostic to use simple input and verify it excludes sibling/other directory fixtures.
- [x] Run relevant regressions, browser checks and final native diagnostic, build/update/restart portable output and update hashes/docs.
