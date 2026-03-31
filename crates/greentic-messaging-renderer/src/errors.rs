use thiserror::Error;

/// Generic renderer error wrapper.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct RendererError(pub String);

impl From<String> for RendererError {
    fn from(value: String) -> Self {
        RendererError(value)
    }
}

impl From<&str> for RendererError {
    fn from(value: &str) -> Self {
        RendererError(value.to_string())
    }
}
