//! Keep-alive на стороне сервера: таймаут ожидания UDP-`PING` от клиента.

use std::time::{Duration, Instant};

pub use crate::protocol::DEFAULT_PING_TIMEOUT_SECS;

/// Возвращает `true`, если с момента `last_ping_at` прошло больше `timeout`.
pub fn ping_deadline_exceeded(last_ping_at: Instant, timeout: Duration) -> bool {
    last_ping_at.elapsed() > timeout
}
