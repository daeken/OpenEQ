//! Loads EverQuest SIDL XML and produces a renderer-neutral, ordered UI draw list.
//!
//! UI files describe presentation only. Game values and input state come from
//! [`UiBindings`]; no script or client command in a UI file is executed.

mod document;
mod layout;

pub use document::{Element, TextureInfo, UiAnimation, UiDocument, UiError, UiImageFrame};
pub use layout::{
    Color, DrawCommand, HitTarget, Rect, TextAlign, TextLine, UiBindings, UiFrame, UiWindow,
    WidgetState,
};
