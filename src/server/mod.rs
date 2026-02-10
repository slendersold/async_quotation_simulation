//! Входная точка серверной логики: запуск TCP-слушателя и управление стримами.

pub mod generator;
pub mod keepalive;
pub mod registry;
pub mod streaming;
pub mod tcp_accept;
pub mod tickers;
