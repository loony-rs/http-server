pub mod radix;

/// Errors that can occur during route registration.
#[derive(Debug, Clone, PartialEq)]
pub enum RouterError {
    /// Two routes share a dynamic path segment but give it different names.
    ///
    /// The radix tree can only store one parameter name per node, so routes
    /// like `/user/:id/...` and `/user/:uid/...` cannot coexist under the
    /// same prefix.
    ConflictingParameterNames {
        existing: String,
        conflicting: String,
    },
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingParameterNames {
                existing,
                conflicting,
            } => write!(
                f,
                "conflicting path parameter names: existing ':{existing}', new ':{conflicting}'"
            ),
        }
    }
}

impl std::error::Error for RouterError {}
