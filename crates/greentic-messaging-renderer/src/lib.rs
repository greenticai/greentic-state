//! Canonical renderer for messaging providers.
//! This crate exposes a minimal render plan model plus a no-op renderer that passes
//! Adaptive Card JSON through unchanged, and a downsampling renderer that converts
//! Adaptive Cards to text based on channel capabilities.

pub mod ac_extract;
pub mod context;
pub mod errors;
pub mod mode;
pub mod plan;
pub mod planner;
pub mod renderer;

pub use ac_extract::extract_planner_card;
pub use context::RenderContext;
pub use errors::RendererError;
pub use mode::RendererMode;
pub use plan::{RenderItem, RenderPlan, RenderTier, RenderWarning};
pub use planner::{PlannerAction, PlannerCapabilities, PlannerCard, plan_render};
pub use renderer::{
    CardRenderer, DownsampleCardRenderer, NoopCardRenderer, render_plan_from_envelope,
};
