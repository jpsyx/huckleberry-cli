//! Numbered selection controls shared by prompts and command menus.
mod draw;
mod model;
pub use draw::render;
pub use model::{MenuItem, Selection, SelectionAction};
