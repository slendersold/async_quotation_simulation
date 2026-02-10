//! Публичные реэкспорты и API библиотеки: объединяет модули, типы и функции для клиента/сервера.

pub mod client;
pub mod error;
pub mod model;
pub mod net;
pub mod protocol;
pub mod server;
pub mod tickers;

pub use crate::error::{Error, Result};
