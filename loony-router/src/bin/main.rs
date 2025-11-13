use loony_router::*;

fn main() {
    fn handler_root() -> String { "root".to_string() }
    fn handler_user() -> String { "user".to_string() }
    fn handler_user_id() -> String { "user_id".to_string() }

    let mut router = RadixRouter::new();
    router.add_route("/", handler_root);
    router.add_route("/user", handler_user);
    router.add_route("/user/:id", handler_user_id);
    router.add_route("/user/:id/get/:username", handler_user_id);

    let routes = vec!["/", "/user", "/user/42", "/unknown", "/user/231/get/sankar"];

    for route in routes {
        match router.find_route(route) {
            Some((handler, params)) => println!("{} -> {}, params = {:?}", route, handler(), params),
            None => println!("{} -> 404", route),
        }
    }
}