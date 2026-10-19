# Search experience improvements

Approved in chat on 2026-10-15 (Asia/Shanghai): improve search relevance, settings interaction and query efficiency together.

## Relevance

Retain the public match tiers (0–4) and successful-history-first ordering. Within a tier, distinguish complete derived application keys from prefixes and substrings: complete full pinyin, complete initials/acronyms, derived-key prefix, then partial matching. A complete `weixin` or `wx` should prefer 微信 over 企业微信 when neither has successful history; `wt` should prefer Windows Terminal over a subsequence in Microsoft PowerToys. Exact title/alias matching, file ranking, pins and usage remain covered by regression tests. No application-specific hardcoded boosts.

## Settings interaction

The launch shortcut field records a modifier plus a supported key. Tab remains normal focus navigation; Escape leaves the field without overwriting the shortcut. Bare keys and unsupported/IME keys produce actionable inline feedback, and modifier-only presses do not overwrite a complete shortcut. Do not claim an OS shortcut is available before native registration succeeds. Keep existing backend validation and atomic rollback.

Application aliases accept English and Chinese commas, trim whitespace and deduplicate using NFKC plus case folding before submission. Validate the existing 16-alias and 128-character limits with feedback next to the input. Preserve the neutral appearance, visible labels, keyboard focus and narrow/dark layouts.

## Query efficiency

Avoid copying/ranking application catalogs for file/folder queries and other filters excluding applications. Reject candidates outside scope before deduplication/title normalization and build deduplication keys only for matches. Reuse prepared normalized query terms within ranking passes where supported by the current architecture; do not add query/history caches or background timers.

## Verification and delivery

Add failing behavioral tests before implementing changes. Run frontend tests/check/build, Rust tests/check and native smoke where required. Compare release workloads on identical inputs with alternating execution order and disclose measurement scope. Inspect changed settings visually in light, dark and narrow layouts. Preserve existing published artifacts and Latest-Release.json until new evidence supports a publication. Produce an implementation report with fresh checks and practical measurement limits.
