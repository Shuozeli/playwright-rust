<!-- agent-updated: 2026-11-23T03:34:47Z -->

# Ownership and lifecycle

Remote CDP is safe only when topology visibility and cleanup authority are kept
separate. Seeing a Context or Page does not mean the attachment owns it.

## Ownership table

| Resource | Owner | Attachment authority |
| --- | --- | --- |
| Remote Chrome process | External launcher/control plane | Disconnect only |
| Pre-existing Context | External profile | Borrow and enumerate |
| Pre-existing Page | External profile | Observe; never close on session shutdown |
| Context created by this attachment | This attachment | Close Context and descendants |
| Page created in a borrowed Context | This attachment | Close the exact Page |
| CDP socket and driver task | This attachment | Retire locally under a bounded deadline |
| External lease | External adapter | Invoke the supplied release hook |

## How ownership is established

Ownership begins only after a successful creation operation is attributed to
the attachment. Enumeration is not attribution. The current implementation
tracks explicit `new_context` and `new_page` completions; popup/opener lineage
remains incomplete.

## Borrowed default Context

Persistent Chrome profiles normally expose a default Context. Playwright Rust
returns it from `Browser::contexts()` with `is_owned() == false`.

- `context.new_page()` creates a session-owned Page while retaining profile
  cookies and login state.
- `context.close()` is rejected because the Context belongs to the profile.
- `browser.close()` retires session-owned Pages but preserves pre-existing
  Pages and the Chrome process.

## Explicit close

The target close sequence is:

```text
OPEN
  -> seal new admissions
  -> drain or fence admitted operations
  -> close exactly owned Pages and Contexts
  -> disconnect transport
  -> join or force-retire local driver tasks
  -> release external lease
  -> CLOSED
```

One absolute graceful deadline and one forced-local deadline should govern the
whole sequence. Repeated close calls should observe one stable shared result.

## Peer disconnect

When the server closes the WebSocket, in-flight work must fail promptly with a
transport/disconnected error. Browser-side cleanup is impossible after lease
revocation, so local shutdown must not wait indefinitely for it.

## Lease hook

`RemoteLease` is deliberately generic. It notifies the external authority
after local cleanup and disconnect have been attempted. It is not permission
to send `Browser.close`.

Current implementations should make release idempotent. Cancellation-safe,
retry-stable release state is an active P1 task.

## Drop

Explicit `browser.close()` is currently the only reliable cleanup contract.
Drop disconnects local state but cannot synchronously guarantee asynchronous
target cleanup or lease release. Applications should structure cleanup like a
`finally` block.

## Required regressions

An attach-safety test must:

1. inventory a pre-existing marker Page;
2. attach a new client;
3. create and exercise one owned target;
4. close the attachment under a timeout;
5. prove the owned target disappeared; and
6. prove the marker and Chrome process remain alive.

Peer-revocation tests must also prove pending calls terminate and local close
does not hang.
