//! Библиотека: протокол, модель данных, сеть и модули клиента и сервера.

pub mod client;
pub mod error;
pub mod model;
pub mod net;
pub mod protocol;
pub mod server;

pub use crate::error::{Error, Result};
