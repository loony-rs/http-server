use std::collections::HashMap;

#[derive(Default)]
pub struct RadixNode {
    // Static path segments: "api", "user", etc.
    pub static_children: HashMap<String, Box<RadixNode>>,
    // Parameter segments: ":id", ":user_id"
    pub param_child: Option<(String, Box<RadixNode>)>,
    // Handler for this route (None for intermediate nodes)
    pub handler: Option<fn() -> String>,
}

pub struct RadixRouter {
    pub root: RadixNode,
}

impl RadixRouter {
    pub fn new() -> Self {
        Self { root: RadixNode::default() }
    }

    pub fn add_route(&mut self, path: &str, handler: fn() -> String) {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        Self::add_to_node(&mut self.root, &segments, handler);
    }

    pub fn add_to_node(node: &mut RadixNode, segments: &[&str], handler: fn() -> String) {
        if segments.is_empty() {
            node.handler = Some(handler);
            return;
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        if segment.starts_with(':') {
            // Parameter segment
            let param_name = segment[1..].to_string();
            if let Some((existing_name, child_node)) = &mut node.param_child {
                if *existing_name != param_name {
                    panic!("Conflicting parameter names");
                }
                Self::add_to_node(child_node, remaining, handler);
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, handler);
                node.param_child = Some((param_name, Box::new(new_child)));
            }
        } else {
            // Static segment
            if let Some(child_node) = node.static_children.get_mut(segment) {
                Self::add_to_node(child_node, remaining, handler);
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, handler);
                node.static_children.insert(segment.to_string(), Box::new(new_child));
            }
        }
    }

    pub fn find_route(&self, path: &str) -> Option<(fn() -> String, HashMap<String, String>)> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut params = HashMap::new();
        Self::find_in_node(&self.root, &segments, &mut params).map(|h| (h, params))
    }

    pub fn find_in_node(
        node: &RadixNode,
        segments: &[&str],
        params: &mut HashMap<String, String>,
    ) -> Option<fn() -> String> {
        if segments.is_empty() {
            return node.handler;
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        // Try static match first
        if let Some(child_node) = node.static_children.get(segment) {
            if let Some(handler) = Self::find_in_node(child_node, remaining, params) {
                return Some(handler);
            }
        }

        // Try parameter match
        if let Some((param_name, child_node)) = &node.param_child {
            params.insert(param_name.clone(), segment.to_string());
            if let Some(handler) = Self::find_in_node(child_node, remaining, params) {
                return Some(handler);
            }
            params.remove(param_name);
        }

        None
    }
}

