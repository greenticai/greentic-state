/// Rendering mode switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RendererMode {
    #[default]
    Passthrough,
    Downsample,
}

impl RendererMode {
    /// Parse a renderer mode string (case-insensitive).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "passthrough" | "noop" => Some(Self::Passthrough),
            "downsample" => Some(Self::Downsample),
            _ => None,
        }
    }

    /// Reads the renderer mode from `GREENTIC_MESSAGING_RENDERER_MODE`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Self {
        std::env::var("GREENTIC_MESSAGING_RENDERER_MODE")
            .ok()
            .and_then(|value| Self::parse(&value))
            .unwrap_or_default()
    }

    /// WASM builds cannot read environment variables; always return default.
    #[cfg(target_arch = "wasm32")]
    pub fn from_env() -> Self {
        Self::default()
    }
}
