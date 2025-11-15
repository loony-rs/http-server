use regex::Regex;
use std::collections::HashMap;

/// One compiled route: original template, compiled regex, and ordered param names.
#[derive(Debug)]
struct Route {
    template: String,
    regex: Regex,
    param_names: Vec<String>, // param name i corresponds to capture group i+1
}

/// Convert a route template like "/user/get/:user_id/:user_name" into a regex string
/// and returns the ordered param names.
fn template_to_regex(template: &str) -> (String, Vec<String>) {
    // split on '/', keep leading slash semantics
    if template == "/" {
        return ("^/$".to_string(), vec![]);
    }

    let mut param_names = Vec::new();
    let mut parts = Vec::new();

    // trim leading/trailing slashes for easier segment handling
    let trimmed = template.trim_matches('/');

    for seg in trimmed.split('/') {
        if seg.starts_with(':') {
            // parameter
            let name = seg.trim_start_matches(':').to_string();
            // capture group matching a single path segment (no slashes)
            parts.push("([^/]+)".to_string());
            param_names.push(name);
        } else {
            // literal segment -> escape regex metacharacters
            parts.push(regex::escape(seg));
        }
    }

    // assemble regex: must match full path; allow optional trailing slash? here we accept exact
    let body = parts.join("/");
    let regex_str = format!("^/{}$", body);
    (regex_str, param_names)
}

/// Build Route structs for each template
fn build_routes<T: AsRef<str>>(templates: &[T]) -> Vec<Route> {
    templates
        .iter()
        .map(|t| {
            let tpl = t.as_ref().to_string();
            let (re_str, param_names) = template_to_regex(&tpl);
            let regex = Regex::new(&re_str).expect("invalid regex built");
            Route {
                template: tpl,
                regex,
                param_names,
            }
        })
        .collect()
}

/// Try to match `path` against the list of routes (in order).
/// If matched, returns (template, params map).
fn match_path(routes: &[Route], path: &str) -> Option<(String, HashMap<String, String>)> {
    for route in routes {
        if let Some(caps) = route.regex.captures(path) {
            let mut params = HashMap::new();
            // capture groups start at 1
            for (i, name) in route.param_names.iter().enumerate() {
                if let Some(mat) = caps.get(i + 1) {
                    params.insert(name.clone(), mat.as_str().to_string());
                }
            }
            return Some((route.template.clone(), params));
        }
    }
    None
}



fn main() {
    // templates given by the user
    let templates = [
        "/",
        "/user/all",
        "/user/get/:user_id",
        "/user/get/:user_id/:user_name",
    ];

    let routes = build_routes(&templates);

    // test paths
    let tests = [
        "/",
        "/user/get/all",
        "/user/get/42",
        "/user/get/42/john-doe",
        "/user/get",                   // no match
        "/user/get/42/extra/segment",  // no match
    ];

    for path in &tests {
        match match_path(&routes, path) {
            Some((template, params)) => {
                println!("{}  -> matched template: {}", path, template);
                if params.is_empty() {
                    println!("   params: (none)");
                } else {
                    println!("   params:");
                    for (k, v) in params.iter() {
                        println!("     {} = {}", k, v);
                    }
                }
            }
            None => {
                println!("{}  -> no match", path);
            }
        }
    }
}
