pub mod aes_registry;
pub mod breaks;
pub mod build;
pub mod color;
pub mod data;
pub mod palettes;
pub mod error;
pub mod geom;
pub mod guides;
pub mod layout;
pub mod position;
pub mod probes;
pub mod scene;
pub mod serde_util;
pub mod scale;
pub mod spec;
pub mod stat;
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
