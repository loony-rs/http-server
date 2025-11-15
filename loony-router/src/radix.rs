use std::collections::HashMap;

#[derive(Default)]
pub struct RadixNode {
    // Static path segments: "api", "user", etc.
    pub static_children: HashMap<String, Box<RadixNode>>,
    // Parameter segments: ":id", ":user_id"
    pub param_child: Option<(String, Box<RadixNode>)>,
    // service_index for this route (None for intermediate nodes)
    pub service_index: Option<usize>,
}

pub struct RadixRouter {
    pub root: RadixNode,
}

impl RadixRouter {
    pub fn new() -> Self {
        Self { root: RadixNode::default() }
    }


    pub fn add_route(&mut self, path: &str, service_index: usize) {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        Self::add_to_node(&mut self.root, &segments, service_index);
    }

    pub fn add_to_node(node: &mut RadixNode, segments: &[&str], service_index: usize) {
        if segments.is_empty() {
            node.service_index = Some(service_index);
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
                Self::add_to_node(child_node, remaining, service_index);
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, service_index);
                node.param_child = Some((param_name, Box::new(new_child)));
            }
        } else {
            // Static segment
            if let Some(child_node) = node.static_children.get_mut(segment) {
                Self::add_to_node(child_node, remaining, service_index);
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, service_index);
                node.static_children.insert(segment.to_string(), Box::new(new_child));
            }
        }
    }

    pub fn find_route(&self, path: &str) -> Option<(usize, HashMap<String, String>)> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut params = HashMap::new();
        Self::find_in_node(&self.root, &segments, &mut params).map(|h| (h, params))
    }

    pub fn find_in_node(
        node: &RadixNode,
        segments: &[&str],
        params: &mut HashMap<String, String>,
    ) -> Option<usize> {

        if segments.is_empty() {
            return node.service_index.clone();
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        // Try static match first
        if let Some(child_node) = node.static_children.get(segment) {
            if let Some(service_index) = Self::find_in_node(child_node, remaining, params) {
                return Some(service_index);
            }
        }

        // Try parameter match
        if let Some((param_name, child_node)) = &node.param_child {
            params.insert(param_name.clone(), segment.to_string());
            if let Some(service_index) = Self::find_in_node(child_node, remaining, params) {
                return Some(service_index);
            }
            params.remove(param_name);
        }

        node.service_index.clone()
    }

}