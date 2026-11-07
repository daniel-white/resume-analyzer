# Live analysis updates

The library and resume detail page subscribe to the ownership-protected
`GET /api/resumes/:id/events` stream using a same-origin `EventSource`.
Only pending analyses open streams; completion or navigation closes them.
There is no periodic polling.

The Rust stream emits named `ResumeUploaded`, `ResumeParsed`,
`AtsRunRequested`, and `AtsCallbackReceived` events. The frontend treats them
as hints and reloads the route's authoritative API state. Refreshes are
serialized, with a follow-up refresh when hints arrive during an existing one.
The PDF/DOCX viewer remains mounted while the surrounding analysis updates.

Events are not replayed. The page refreshes state on every stream open,
including automatic browser reconnection, to recover updates missed during
disconnects or between initial page loading and subscribing. Stream errors
also reconcile state so revoked sessions and removed resumes reach existing
route error handling.

Vite and the deployed gateway route `/api` to Rust directly. A TanStack
streaming server route supports the standalone production frontend using
runtime `API_URL`, forwarding cookies and `Last-Event-ID`, disabling caching,
and passing through the response body without buffering.

## Checks

In `web/`, run `pnpm test`, `pnpm typecheck`, `pnpm test:e2e`, and
`pnpm test:e2e:production`. Browser tests cover named-event updates, quiet
streams without polling, reconnect recovery without an event, library updates,
cleanup, and authorization.

## Existing backend limitation

The event bus is in-process. SSE needs the request and dispatching worker in
the same API process (the current single-replica demo). Multiple API replicas
need shared event delivery; persistent database state alone does not broadcast
live hints to another replica. Reconnection recovery does not replace that.
