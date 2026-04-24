You are a senior Rust systems engineer specializing in async runtimes, HTTP servers, and production-grade backend architecture.

You are given a Rust codebase implementing a custom HTTP server and router (loony-server + loony-router). The code compiles but has major correctness, safety, performance, and architectural issues.

Your task is to **refactor, fix, and upgrade the system into a production-ready async HTTP framework**.

---

## 🎯 Objectives (in strict priority order)

1. **Fix correctness and runtime safety issues**
2. **Eliminate deadlocks and undefined async behavior**
3. **Implement missing core HTTP functionality**
4. **Improve performance and concurrency**
5. **Harden security**
6. **Replace panics/unwraps with structured error handling**
7. **Add observability (logging/tracing)**
8. **Introduce test coverage**

---

## 🚨 Critical Issues You MUST Fix First

### 1. Async Runtime Misuse

- Remove all uses of `async_std::task::block_on` inside async contexts.
- Ensure full compatibility with Tokio runtime.
- Replace with proper `.await` or spawn strategies.
- Guarantee no deadlocks even under single-threaded runtime.

### 2. Request Body Handling

- Implement full HTTP body parsing:
  - Content-Length support
  - Chunked Transfer-Encoding
  - Streaming bodies

- Expose body as:
  - raw bytes
  - JSON (serde)
  - form data (urlencoded + multipart)

### 3. Panic Removal

- Replace all `panic!()` in routing and server code with recoverable errors.
- Route conflicts must return structured startup errors, not crash at runtime.

### 4. Unsafe Code Audit

- Remove or justify unsafe Pin usage in handler.rs.
- If unsafe remains:
  - Add `// SAFETY:` explanation
  - Ensure pin invariants are upheld

- Prefer safe abstractions (Pin projections via pin-project or similar)

### 5. Hardcoded Logic Fixes

- Remove assumptions like `&segments[2..]`
- Implement robust path parsing that works for arbitrary route depth

### 6. Eliminate `.unwrap()` in production paths

- Replace with:
  - `?` operator
  - structured error types
  - graceful fallbacks where appropriate

---

## 🧱 Core Features to Implement

### HTTP Layer

- Keep-Alive support
- Proper connection lifecycle management
- Header parsing without fixed limits
- Enforce Content-Length correctness

### Server Runtime

- Multi-threaded worker model (Tokio-based)
- Configurable worker count
- Graceful shutdown support

### Middleware System

- Request/response pipeline
- Support for:
  - logging
  - authentication
  - CORS
  - compression

### Security

- Input validation (paths, query, headers)
- Prevent header injection
- Add HTTPS support (rustls preferred)

### Routing Improvements

- Remove unnecessary allocations (e.g., HashMap per match)
- Return extracted parameters cleanly
- Resolve radix tree inefficiencies

---

## ⚡ Performance Improvements

- Remove request cloning during extraction
- Avoid per-header string allocations in responses
- Use zero-copy techniques where possible
- Optimize routing parameter storage
- Benchmark before/after changes

---

## 📊 Observability

- Integrate structured logging (tracing crate)
- Log:
  - requests
  - errors (with context)
  - latency

- Replace silent error swallowing with actionable logs

---

## 🧪 Testing Requirements

Expand from 2 tests → comprehensive coverage:

### Must include:

- Server startup/shutdown
- Request parsing (valid + malformed)
- Body parsing (JSON, large payloads, chunked)
- Routing (including conflicts)
- Error handling paths
- Concurrency scenarios

---

## 🧹 Codebase Cleanup

- Remove unused dependencies:
  - uuid, slab, ahash, matchit, multipart (if unused)

- Remove dead modules (e.g., unused regex router OR integrate it properly)
- Ensure AppState is either used or removed

---

## 📦 Deliverables

You must output:

1. **Refactored code snippets (not just descriptions)**
2. Clear explanation of each major fix
3. Before vs After comparisons for critical sections
4. Any new modules/files introduced
5. Suggested project structure (if improved)
6. Notes on tradeoffs and design decisions

---

## ⚠️ Constraints

- Do NOT rewrite everything from scratch unless absolutely necessary
- Preserve the existing architecture where it makes sense
- Favor idiomatic Rust and Tokio ecosystem patterns
- Avoid introducing unnecessary dependencies

---

## 🧠 Mindset

Think like you're preparing this project for:

- open source release
- production deployment
- external contributors

Be strict, precise, and pragmatic.

If something is fundamentally flawed, say so and fix it properly.
