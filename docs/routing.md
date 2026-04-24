# Routing

## Overview

Routes are registered on a `Router` using free functions from the `route` module, then passed to `App::routes`. The underlying router uses a **radix tree** for O(path-depth) lookup with no per-request allocation for static segments.

## HTTP Methods

```rust
use loony_server::{route, router::Router};

fn routes() -> Router {
    Router::new()
        .route(route::get("/users").to(list_users))
        .route(route::post("/users").to(create_user))
}
```

Available helpers: `route::get`, `route::post`. Additional methods can be added by constructing a `Route` directly.

## Path Parameters

Dynamic segments start with `:`. The values are extracted in order via the `Path<T>` extractor.

```rust
use loony_server::{Path, responder::Responder, route, router::Router};

// Single param: /user/:id
async fn get_user(Path(id): Path<i32>) -> impl Responder {
    format!("user {id}")
}

// Two params: /user/:user_id/:user_name
async fn get_user_name(Path((uid, name)): Path<(i32, String)>) -> impl Responder {
    format!("user {uid} is {name}")
}

fn routes() -> Router {
    Router::new()
        .route(route::get("/user/:id").to(get_user))
        .route(route::get("/user/:id/:name").to(get_user_name))
}
```

### Supported `Path<T>` types

| Type | Example path | Captures |
|---|---|---|
| `Path<i32>` | `/user/:id` | first segment as integer |
| `Path<String>` | `/file/:name` | first segment as string |
| `Path<(i32, String)>` | `/user/:id/:name` | id, name |
| `Path<(i32, i32)>` | `/range/:from/:to` | from, to |
| `Path<(String, String)>` | `/a/:x/:y` | x, y |
| `Path<(i32, String, String)>` | `/a/:id/:x/:y` | id, x, y |

### Implementing custom path types

```rust
use loony_server::extract::FromPathSegments;

#[derive(Clone)]
struct Pagination { page: i32, size: i32 }

impl FromPathSegments for Pagination {
    fn from_segments(segments: &[&str]) -> Option<Self> {
        Some(Pagination {
            page: segments.get(0)?.parse().ok()?,
            size: segments.get(1)?.parse().ok()?,
        })
    }
}
```

## Scopes

Group routes under a common prefix using `route::scope`:

```rust
use loony_server::{route, router::Router};

fn routes() -> Router {
    Router::new()
        .route(route::get("/").to(index))
        .service(
            route::scope("/api/v1")
                .route(route::get("/users").to(list_users))
                .route(route::post("/users").to(create_user))
                .route(route::get("/users/:id").to(get_user)),
        )
        .service(
            route::scope("/admin")
                .route(route::get("/stats").to(stats)),
        )
}
```

The scope prefix is prepended to each child route's path automatically.

## Nested Scopes

Scopes can be nested by adding a scope as a service inside another scope. The paths are concatenated at registration time.

## Static vs Dynamic Segments

Static segments (e.g., `/user/all`) are always tried before dynamic segments (e.g., `/user/:id`) at the same depth. This means:

```
GET /user/all    → matches route::get("/user/all")   (static wins)
GET /user/42     → matches route::get("/user/:id")   (dynamic fallback)
```

## Route Conflicts

Registering two dynamic segments at the same position with different names is a startup error:

```rust
// ERROR at startup — ":id" and ":uid" conflict at the same position
router.add_route("/user/:id", ...)
router.add_route("/user/:uid", ...)
```

The server will log the error and exit cleanly rather than silently picking one.

## Accessing the Raw URI

Handlers can receive the raw request URI as a plain `String` argument:

```rust
async fn debug_path(uri: String) -> impl Responder {
    format!("you requested: {uri}")
}
```

## Full Example

```rust
use loony_server::{
    App, HttpServer, Json, Path,
    extract::Data,
    responder::Responder,
    response::HttpResponse,
    route, router::Router,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct Db;

#[derive(Deserialize)]
struct NewItem { name: String }

#[derive(Serialize)]
struct Item { id: i32, name: String }

async fn list(Data(_db): Data<Db>) -> impl Responder {
    HttpResponse::ok().json(vec![Item { id: 1, name: "thing".into() }]).unwrap()
}

async fn get(Path(id): Path<i32>) -> impl Responder {
    format!("item {id}")
}

async fn create(Data(_db): Data<Db>, Json(body): Json<NewItem>) -> impl Responder {
    format!("created: {}", body.name)
}

fn routes() -> Router {
    Router::new().service(
        route::scope("/items")
            .route(route::get("").to(list))
            .route(route::get("/:id").to(get))
            .route(route::post("").to(create)),
    )
}

#[tokio::main]
async fn main() {
    HttpServer::new(move || App::new().data(Db).routes(routes))
        .bind(3000)
        .run()
        .await
        .unwrap();
}
```
