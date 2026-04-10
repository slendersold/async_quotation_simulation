//! Серверная логика: TCP-команды, хаб котировок, UDP-стриминг.

pub mod generator;
pub mod keepalive;
pub mod registry;
#[cfg(feature = "cli")]
pub mod run;
pub mod streaming;
pub mod tcp_accept;
pub mod tickers;
