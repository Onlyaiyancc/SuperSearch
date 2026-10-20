# 0.1.7 candidate user acceptance feedback

Source: the user's manual acceptance report in this chat. Exact running executable hash was not independently rechecked. Findings refer to the supplied 0.1.7 candidate and must not be presented as an agent-observed physical run.

| Item | User result |
| --- | --- |
| Initial startup/index state | Passed: readiness updates without reopening settings. Failed: pressing Restart search engine displays unavailable/timeout; another press has the same result. |
| IME and file opening | Passed. |
| Shortcut and single instance | Passed. |
| Settings persistence and search recovery | Passed. |
| Display unplug | Failed: recall remains possible, but size is incorrect until process restart. This report does not establish separate 150%/200% DPI results. |
| Sleep/wake | Passed. |
| Interactive installer and missing WebView2 | Deferred by the user. |

Additional confirmed requirement: configurable recall on the mouse pointer's monitor, centered within that monitor's work area; keep remembered positioning as the default alternative.

Public release remains on hold for the known restart and geometry defects. The original candidate delivery is retained as historical evidence. A replacement candidate must repeat these two failing interactions; previous passes must not be claimed as physical verification of the new executable.

## Replacement implementation and preview checks

The replacement keeps in-progress engine status through the worker's 250ms reply deadline. Exact SDK IPC timeouts receive at most 30 seconds of startup grace, ending on the first ready response; service and process failures stay visible. Window fitting restores the canonical 680 logical-pixel width instead of deriving it from stale physical geometry. Recall and visible DPI changes force layout synchronization. The new General settings option defaults to remembered placement and can center recall on the mouse pointer's monitor.

Read-only code review found an additional SDK-timeout path; it was fixed and re-reviewed with no remaining blockers. Regression tests cover timeout grace expiry, readiness ending grace, genuine failure visibility, geometry, forced layout synchronization, and setting persistence/default compatibility.

The local browser preview's accessibility tree verified the visible Chinese label, both placement options, keyboard selection, saving and persistence after reopening settings. Screenshot capture failed through both documented browser capture APIs, so this is semantic interaction evidence, not completed visual screenshot QA. Physical display unplugging, mixed-DPI recall and mouse-monitor placement still require a physical recheck. Existing 30-minute measurements apply only to the original candidate binary.

## Candidate 2 user recheck

The user reported A (engine restart) and C (mouse-monitor recall setting) passed. B now has normal dimensions, but remembered positioning leaves the recalled window stuck at the upper-left of the primary display after the secondary display is unplugged. This is user-reported physical evidence; the exact running hash was not independently reconfirmed. Complete the disconnected-display position fallback and request a B-only recheck on candidate 3. Keep the prior candidate and evidence; no public release yet.
