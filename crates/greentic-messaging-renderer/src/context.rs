use serde::{Deserialize, Serialize};

/// Context supplied to the renderer about the target environment.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderContext {
    pub target: Option<String>,
}

impl RenderContext {
    pub fn new(target: Option<String>) -> Self {
        Self { target }
    }
}
