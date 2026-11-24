<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Support matrix

This matrix describes product scope, not every low-level capability crate.

## Supported remote-CDP slice

| Area | Status | Evidence |
| --- | --- | --- |
| Chromium CDP attach | Supported, pre-release | Unit, mock transport, and live Chrome tests |
| HTTP discovery and WebSocket URL | Supported | Focused connection tests |
| Custom handshake headers | Supported when feature-selected | Mock upgrade verifies forwarding |
| Borrowed default Context | Supported | Live two-attachment profile-state test |
| Explicit owned Context/Page cleanup | Supported | Live marker-target preservation test |
| Navigation and rendered HTML | Supported | Live scraper workflow |
| JavaScript evaluation | Supported for expression results | Focused host and live tests |
| Locator wait/click/fill/text/count | Supported focused subset | Unit and live workflow tests |
| Screenshot | Supported focused path | Nonempty live screenshot assertion |
| Peer disconnect | Supported for tested pending calls | Revoked-proxy regression |
| External lease hook | Supported on explicit close | Exactly-once happy-path regression |

## Incomplete behavior

| Area | Current gap |
| --- | --- |
| Popup/indirect targets | Creation lineage is not attributed to the attachment |
| Close cancellation | Guard is not installed before every cancellable phase |
| Close deadline | Some timeouts can multiply by target or phase |
| Lease failure/retry | Repeated close does not yet observe a stable shared release result |
| Drop | Explicit close remains required for reliable cleanup and lease release |
| Error taxonomy | Some target and transport failures collapse into broad errors |
| Rust dependency closure | High-level facade still reaches the broad Node host |
| Upstream API parity | Page, Context, Locator, and option surfaces remain partial |

## Intentional non-goals

- Firefox and WebKit backends.
- A bundled browser download.
- BCP-specific protos, acquisition, renewal, or fencing logic.
- Claiming compatibility from static method-name overlap.
- Hiding unsupported options behind silent fallback behavior.

## How to interpret evidence

| Evidence type | What it proves |
| --- | --- |
| Contract/unit test | One Rust-owned behavior under controlled inputs |
| Mock transport test | Wire/header/terminal behavior without a real browser |
| Live Chrome test | The declared workflow against the dedicated Chromium fixture |
| Adapted upstream test | A bounded semantic comparison with changed setup |
| Unchanged upstream test | The exact pinned row/file against candidate and oracle |

No individual row proves complete Playwright compatibility. Historical results
live under [evidence](../evidence/README.md).
