use crate::RouterError;

#[derive(Default)]
pub struct RadixNode {
    pub static_children: Vec<(String, Box<RadixNode>)>,
    pub param_child: Option<(String, Box<RadixNode>)>,
    pub service_index: Option<usize>,
}

pub struct RadixRouter {
    pub root: RadixNode,
}

impl RadixRouter {
    pub fn new() -> Self {
        Self {
            root: RadixNode::default(),
        }
    }

    /// Insert a route into the tree.
    ///
    /// Returns `Err(RouterError::ConflictingParameterNames)` if a dynamic
    /// segment at the same position already exists under a different name.
    /// The caller is responsible for aborting startup cleanly on error.
    pub fn add_route(&mut self, path: &str, service_index: usize) -> Result<(), RouterError> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        Self::add_to_node(&mut self.root, &segments, service_index)
    }

    fn add_to_node(
        node: &mut RadixNode,
        segments: &[&str],
        service_index: usize,
    ) -> Result<(), RouterError> {
        if segments.is_empty() {
            node.service_index = Some(service_index);
            return Ok(());
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        if segment.starts_with(':') {
            let param_name = &segment[1..];
            if let Some((existing_name, child_node)) = &mut node.param_child {
                if existing_name != param_name {
                    return Err(RouterError::ConflictingParameterNames {
                        existing: existing_name.clone(),
                        conflicting: param_name.to_string(),
                    });
                }
                Self::add_to_node(child_node, remaining, service_index)?;
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, service_index)?;
                node.param_child = Some((param_name.to_string(), Box::new(new_child)));
            }
        } else {
            if let Some(pos) = node.static_children.iter().position(|(k, _)| k == segment) {
                Self::add_to_node(&mut node.static_children[pos].1, remaining, service_index)?;
            } else {
                let mut new_child = RadixNode::default();
                Self::add_to_node(&mut new_child, remaining, service_index)?;
                node.static_children.push((segment.to_string(), Box::new(new_child)));
            }
        }

        Ok(())
    }

    /// Returns the service index and param values in path-order.
    pub fn find_route(&self, path: &str) -> Option<(usize, Vec<String>)> {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut params = Vec::new();
        Self::find_in_node(&self.root, &segments, &mut params).map(|idx| (idx, params))
    }

    fn find_in_node(
        node: &RadixNode,
        segments: &[&str],
        params: &mut Vec<String>,
    ) -> Option<usize> {
        if segments.is_empty() {
            return node.service_index;
        }

        let segment = segments[0];
        let remaining = &segments[1..];

        // Try static match first
        if let Some((_, child_node)) = node.static_children.iter().find(|(k, _)| k == segment) {
            if let Some(service_index) = Self::find_in_node(child_node, remaining, params) {
                return Some(service_index);
            }
        }

        // Try parameter match
        if let Some((_param_name, child_node)) = &node.param_child {
            params.push(segment.to_string());
            if let Some(service_index) = Self::find_in_node(child_node, remaining, params) {
                return Some(service_index);
            }
            params.pop();
        }

        node.service_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RouterError;

    #[test]
    fn add_route_ok() {
        let mut router = RadixRouter::new();
        assert!(router.add_route("/user/:id", 0).is_ok());
        assert!(router.add_route("/user/:id/posts", 1).is_ok());
    }

    #[test]
    fn add_route_conflict_returns_err_not_panic() {
        let mut router = RadixRouter::new();
        router.add_route("/user/:id", 0).unwrap();
        let err = router.add_route("/user/:uid", 1).unwrap_err();
        assert_eq!(
            err,
            RouterError::ConflictingParameterNames {
                existing: "id".to_string(),
                conflicting: "uid".to_string(),
            }
        );
    }

    #[test]
    fn find_route_static() {
        let mut router = RadixRouter::new();
        router.add_route("/health", 0).unwrap();
        let (idx, params) = router.find_route("/health").unwrap();
        assert_eq!(idx, 0);
        assert!(params.is_empty());
    }

    #[test]
    fn find_route_with_one_param() {
        let mut router = RadixRouter::new();
        router.add_route("/user/:id", 0).unwrap();
        let (idx, params) = router.find_route("/user/42").unwrap();
        assert_eq!(idx, 0);
        assert_eq!(params, vec!["42".to_string()]);
    }

    #[test]
    fn find_route_with_two_params() {
        let mut router = RadixRouter::new();
        router.add_route("/user/get/:user_id/:user_name", 0).unwrap();
        let (idx, params) = router.find_route("/user/get/7/alice").unwrap();
        assert_eq!(idx, 0);
        assert_eq!(params, vec!["7".to_string(), "alice".to_string()]);
    }

    #[test]
    fn find_route_no_match_returns_none() {
        let mut router = RadixRouter::new();
        router.add_route("/user/:id", 0).unwrap();
        assert!(router.find_route("/missing").is_none());
    }
}
