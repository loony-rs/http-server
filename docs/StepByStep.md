You are a senior Rust backend engineer tasked with incrementally fixing and upgrading a custom HTTP server and router (loony-server + loony-router).

You must proceed **step-by-step**, completing one phase fully before moving to the next. Do NOT skip steps. Do NOT batch everything into one response.

Each step must include:

- Explanation of the issue
- Exact code changes (diffs or full snippets)
- Why the fix is correct
- Any tradeoffs

If a step introduces risk, call it out.

---

# 🔧 Execution Plan

## STEP 1 — Eliminate Async Runtime Violations (CRITICAL)

### Goals:

- Remove all `async_std::task::block_on` usage
- Ensure everything runs correctly on Tokio

### Tasks:

- Replace blocking calls with `.await`
- If sync boundary exists, refactor function signatures to async
- Use `tokio::spawn` where concurrency is needed
- Verify no deadlocks in single-thread runtime

### Output:

- Updated server.rs and route.rs snippets
- Explanation of why previous code could deadlock

STOP after completing this step.

---

## STEP 2 — Remove Panics and unwraps in Core Paths

### Goals:

- Replace all `.unwrap()` and `panic!()` in:
  - routing
  - connection handling
  - request parsing

### Tasks:

- Introduce a proper error enum if missing
- Convert failures into `Result<T, E>`
- Ensure route conflicts return startup errors instead of panicking

### Output:

- Refactored radix router conflict handling
- Example of improved error propagation

STOP after completing this step.

---

## STEP 3 — Implement Request Body Parsing

### Goals:

- Fully support HTTP request bodies

### Tasks:

- Parse:
  - Content-Length bodies
  - Chunked Transfer-Encoding

- Expose body as:
  - bytes
  - JSON (serde)

- Validate Content-Length correctness

### Output:

- connection.rs changes
- new body parsing module (if needed)
- example handler using JSON body

STOP after completing this step.

---

## STEP 4 — Fix Path Extraction Logic

### Goals:

- Remove hardcoded segment slicing like `&segments[2..]`

### Tasks:

- Implement robust path segmentation
- Ensure no panics on short paths
- Align extraction with router structure

### Output:

- extract.rs fix
- explanation of correct path handling

STOP after completing this step.

---

## STEP 5 — Enable Multi-threaded Runtime

### Goals:

- Replace single-thread loop with scalable worker model

### Tasks:

- Use Tokio multi-thread runtime
- Replace `for _ in 0..1` with configurable workers
- Ensure safe shared state handling (Arc, etc.)

### Output:

- server.rs worker model refactor

STOP after completing this step.

---

## STEP 6 — Implement HTTP Keep-Alive

### Goals:

- Support multiple requests per connection

### Tasks:

- Parse `Connection` headers
- Keep socket open when appropriate
- Handle timeouts and connection reuse

### Output:

- connection loop refactor

STOP after completing this step.

---

## STEP 7 — Add Middleware System

### Goals:

- Introduce request/response pipeline

### Tasks:

- Define middleware trait
- Support chaining
- Add example:
  - logging middleware
  - simple CORS handler

### Output:

- middleware module
- usage example

STOP after completing this step.

---

## STEP 8 — Add Observability

### Goals:

- Make debugging possible

### Tasks:

- Integrate `tracing`
- Log:
  - incoming requests
  - errors (with context)
  - response status + latency

### Output:

- logging setup
- example logs

STOP after completing this step.

---

## STEP 9 — Performance Optimizations

### Goals:

- Remove unnecessary allocations and clones

### Tasks:

- Avoid request cloning
- Replace HashMap in routing with more efficient structure
- Optimize header building (avoid format! per header)

### Output:

- before/after comparisons
- explanation of improvements

STOP after completing this step.

---

## STEP 10 — Security Hardening

### Goals:

- Fix major vulnerabilities

### Tasks:

- Validate headers and inputs
- Prevent header injection
- Enforce Content-Length correctness
- Add optional HTTPS support (rustls)

### Output:

- security fixes with code examples

STOP after completing this step.

---

## STEP 11 — Testing Expansion

### Goals:

- Move from 2 tests → meaningful coverage

### Tasks:

- Add tests for:
  - routing
  - malformed requests
  - body parsing
  - concurrency

- Use async test framework

### Output:

- sample test cases

STOP after completing this step.

---

## STEP 12 — Cleanup Dead Code & Dependencies

### Goals:

- Remove unused complexity

### Tasks:

- Remove unused crates:
  - uuid, slab, ahash, matchit, multipart (if unused)

- Remove or integrate regex router
- Clean unused AppState fields

### Output:

- cleaned Cargo.toml
- explanation

STOP after completing this step.

---

# ⚠️ Rules

- Do NOT jump ahead
- Do NOT summarize all steps at once
- Focus deeply on ONE step per response
- Prefer correctness over cleverness
- Use idiomatic Rust

---

# 🧠 End Goal

A stable, production-ready async HTTP framework with:

- correct async behavior
- real HTTP support
- strong error handling
- observability
- scalability
