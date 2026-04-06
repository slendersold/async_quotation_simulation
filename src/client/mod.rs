//! Входная точка клиентской логики: настройка и запуск компонентов.

pub mod ping;
#[cfg(feature = "cli")]
pub mod run;
pub mod tcp_command;
pub mod udp_recv;
