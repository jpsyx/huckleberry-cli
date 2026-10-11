//! Retained drafts and navigation through the views of a form.
mod model;
mod view;

pub use model::{FormEvent, FormModel, FormState, FormValue, ViewId};
pub use view::{ControlEvent, View};
