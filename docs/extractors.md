# Extractors

Extractors are types that implement `FromRequest`. When a handler declares them as parameters, the framework resolves them from the incoming `ServiceRequest` before calling the handler. If extraction fails, the handler receives a `500 Internal Server Error`.

## Data\<T\> — Shared Application State

`Data<T>` extracts a value that was registered via `App::data()` or `App::app_data()`. The value must be `Clone + Send + Sync + 'static`.

### Registering state

```rust
#[derive(Clone)]
struct Config {
    db_url: String,
    secret: String,
}

HttpServer::new(move || {
    App::new()
        .data(Config {
            db_url: "postgres://localhost/mydb".into(),
            secret: "s3cret".into(),
        })
        .routes(routes)
})
```

### Using state in a handler

```rust
use loony_server::extract::Data;

async fn handler(Data(cfg): Data<Config>) -> impl Responder {
    format!("connecting to {}", cfg.db_url)
}
```

Multiple state types can be registered independently and extracted individually:

```rust
#[derive(Clone)] struct DbPool { /* ... */ }
#[derive(Clone)] struct Mailer { /* ... */ }

App::new()
    .data(DbPool { /* ... */ })
    .data(Mailer { /* ... */ })
```

```rust
async fn handler(
    Data(pool): Data<DbPool>,
    Data(mailer): Data<Mailer>,
) -> impl Responder {
    // use pool and mailer
}
```

## Path\<T\> — URL Path Parameters

`Path<T>` extracts dynamic segments from the request URL in the order they appear in the route pattern.

```rust
use loony_server::Path;

// Route: /user/:id
async fn get_user(Path(id): Path<i32>) -> impl Responder {
    format!("user {id}")
}

// Route: /order/:shop/:id
async fn get_order(Path((shop, id)): Path<(String, i32)>) -> impl Responder {
    format!("order {id} from {shop}")
}
```

See [Routing — Path Parameters](routing.md#path-parameters) for the full list of supported tuple types and how to implement `FromPathSegments` for custom types.

## Json\<T\> — Request Body Deserialization

`Json<T>` deserializes the request body as JSON into type `T`. `T` must implement `serde::DeserializeOwned`.

```rust
use loony_server::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    email: String,
}

// Route: POST /users
async fn create_user(Json(body): Json<CreateUser>) -> impl Responder {
    format!("creating user: {}", body.name)
}
```

If the body is missing or cannot be deserialized, the framework returns `500`. A future improvement (Step 10) will return `400 Bad Request` with a descriptive error.

## String — Raw Request URI

Declaring a plain `String` parameter gives the full raw request URI (path + query string):

```rust
async fn debug(uri: String) -> impl Responder {
    format!("URI: {uri}")
}
```

## Combining Extractors

Multiple extractors can be declared as separate parameters. The framework resolves all of them before calling the handler:

```rust
use loony_server::{Json, Path, extract::Data};
use serde::Deserialize;

#[derive(Clone)]
struct Db { /* ... */ }

#[derive(Deserialize)]
struct UpdateBody { name: String }

// Route: PUT /user/:id
async fn update_user(
    Data(db): Data<Db>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateBody>,
) -> impl Responder {
    format!("updating user {id} with name '{}'", body.name)
}
```

### Supported combinations

The framework provides `FromRequest` implementations for these tuples out of the box:

| Tuple | Description |
|---|---|
| `(Data<T>,)` | single state value |
| `(Data<T>, String)` | state + URI |
| `(Data<T>, Path<P>)` | state + path params |
| `(Data<T>, Json<U>)` | state + JSON body |

For other combinations, implement `FromRequest` directly.

## Implementing a Custom Extractor

```rust
use loony_server::extract::FromRequest;
use loony_server::service::ServiceRequest;
use std::future::{Ready, ready};

#[derive(Clone)]
struct RequestId(String);

impl FromRequest for RequestId {
    type Future = Ready<Result<Self, ()>>;

    fn from_request(req: &ServiceRequest) -> Self::Future {
        let id = req.req.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-request-id"))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        ready(Ok(RequestId(id)))
    }
}

// Now use it in any handler:
async fn handler(RequestId(id): RequestId) -> impl Responder {
    format!("request id: {id}")
}
```
