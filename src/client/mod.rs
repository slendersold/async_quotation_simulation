//! Клиентская логика: TCP-команды, UDP-приём, keep-alive.

pub mod ping;
#[cfg(feature = "cli")]
pub mod run;
pub mod tcp_command;
pub mod udp_recv;
