pub mod aes_registry;
pub mod build;
pub mod data;
pub mod error;
pub mod layout;
pub mod scene;
pub mod serde_util;
pub mod scale;
pub mod spec;
pub mod text;
pub mod theme;

pub mod prelude {
    pub use crate::build::build;
    pub use crate::data::{Column, Dataset};
    pub use crate::error::JplotError;
    pub use crate::layout::layout;
    pub use crate::scene::Scene;
    pub use crate::spec::*;
}

pub use prelude::*;
