//! Периодическая отправка UDP-`PING` на адрес сервера (тот же сокет, с которого приходят котировки).

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::protocol::{DEFAULT_PING_INTERVAL_SECS, PING_COMMAND};

/// Интервал UDP-Ping по умолчанию (~2 с).
pub const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(DEFAULT_PING_INTERVAL_SECS);

/// Ячейка, куда при первом UDP-пакете от сервера записывается его адрес (`recv_from`).
pub type ServerAddrCell = Arc<Mutex<Option<SocketAddr>>>;

/// Фоновый цикл: раз в `interval` шлёт `PING` на адрес из `server_addr`, пока `stop` не выставлен.
pub fn spawn_udp_ping_loop(
    socket: UdpSocket,
    server_addr: ServerAddrCell,
    interval: Duration,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(interval);
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let target = {
                let Ok(guard) = server_addr.lock() else {
                    continue;
                };
                match *guard {
                    Some(a) => a,
                    None => continue,
                }
            };
            let _ = socket.send_to(PING_COMMAND.as_bytes(), target);
        }
    })
}
