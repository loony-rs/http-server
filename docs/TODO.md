### Critical Blockers (Compilation / Correctness)

1. block_on() inside async context — loony-server/src/server.rs:75, :124, route.rs:93
   Uses async_std::task::block_on() on a tokio runtime. This can deadlock on single-threaded runtimes and kills any concurrency benefit.

2. No request body reading — loony-server/src/connection.rs
   The server reads headers and stops. POST/PUT body content is silently discarded. No JSON payload, no form data, nothing.

3. Panic on route conflict — loony-router/src/radix.rs:39
   panic!("Conflicting parameter names") — a misconfigured route crashes the entire server instead of returning an error.

4. Hardcoded segment skip — loony-server/src/extract.rs:145
   &segments[2..] — skips exactly 2 path segments unconditionally. Will panic if segments < 2, and gives wrong results for routes with different nesting depth.

5. 33 .unwrap() calls in production paths — including on TCP accept, request parse, pool creation, JSON build.

### Major Missing Features

Feature Status
HTTP Keep-Alive Missing — every connection closes after 1 request
Request body parsing Missing — POST/PUT bodies ignored
Chunked Transfer-Encoding Missing
HTTPS/TLS Missing
Middleware system Missing
CORS Missing
Cookies Missing
Static file serving Missing
Multi-threaded workers Missing — for \_ in 0..1 in server.rs:183
Graceful shutdown Missing
Rate limiting Missing
Request logging Missing
Compression (gzip/deflate) Missing
Websockets Missing

### Security Problems

No input validation — path parameters, query strings passed through raw with no sanitization.
No HTTPS — all data transmitted in plaintext.
Header injection — headers inserted into responses without validation. Location: header in redirects could be attacker-controlled.
Content-Length not enforced — connection.rs:131-135 — body size not validated, mismatches not caught.
Fixed 16-header limit — request.rs:34 — requests with >16 headers silently lose the rest.

### Performance Problems

Single-threaded — one worker loop, one connection at a time.
Request cloned on every poll — extract.rs:210, extract.rs:254.
HashMap allocated on every route match — radix.rs:61, parameters stored in a HashMap even though they are never returned to the caller.
String allocation per header on response build — response.rs:504, format!() per header.

### Error Handling

Errors caught and silently swallowed: server.rs:79 returns 500 with no logging of what actually failed.
No logging/tracing at all. When something goes wrong you have no visibility.
error.rs has a good error hierarchy but it's largely not used — most code just .unwrap().

### Dead / Incomplete Code

Regex router — loony-router/src/regex_based.rs — fully implemented, never wired up.
multipart dependency — in Cargo.toml, never used.
AppState.name — src/main.rs:27-30 — defined and injected but never read in any handler.
uuid, slab, ahash, matchit — declared in Cargo.toml, unused.

### Tests

2 tests total. That's it.

route.rs:315 — basic route matching
resource.rs:106 — basic resource handling
No tests for: server startup, connection handling, path extraction, error cases, malformed requests, large payloads, routing conflicts, status codes, response building.

### Unsafe Code

handler.rs:131 has manual Pin projection using unsafe. This is a correctness minefield — Pin projections have strict invariants (must not move the value, must not expose references that outlive the pin). There is no SAFETY: comment explaining why this is sound.

Bottom line: The architecture is clean and the routing abstractions are well thought out. The main problems in priority order are: (1) no body parsing, (2) block_on in async, (3) single-threaded only, (4) no keep-alive, (5) near-zero test coverage, (6) no error observability. Fix those and the project gets a lot more credible.
