//! Приём UDP-котировок и связка с фоновым [`crate::client::ping`].

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::StockQuote;
use crate::net;
use crate::protocol::Command;

use super::ping::{self, ServerAddrCell};

const UDP_RECV_BUF: usize = 2048;
const UDP_READ_TIMEOUT: Duration = Duration::from_millis(150);

/// `bind` + [`receive_quotes_with_ping_on_socket`].
pub fn receive_quotes_with_ping(
    bind_addr: SocketAddr,
    run_for: Duration,
    ping_interval: Duration,
) -> io::Result<Vec<StockQuote>> {
    let socket = net::udp_bind(bind_addr)?;
    receive_quotes_with_ping_on_socket(socket, run_for, ping_interval)
}

/// Приём на уже привязанном сокете (типичный порядок: `bind`, затем TCP `STREAM`).
///
/// Адрес сервера для `PING` берётся из первой успешно разобранной котировки. Строки `PING`/`PONG`
/// протокола отбрасываются.
pub fn receive_quotes_with_ping_on_socket(
    socket: UdpSocket,
    run_for: Duration,
    ping_interval: Duration,
) -> io::Result<Vec<StockQuote>> {
    let mut quotes = Vec::new();
    receive_quotes_with_ping_on_socket_with_cb(socket, run_for, ping_interval, |q| {
        quotes.push(q.clone());
    })?;
    Ok(quotes)
}

/// Вариант [`receive_quotes_with_ping_on_socket`] с колбэком на каждую котировку.
pub fn receive_quotes_with_ping_on_socket_with_cb(
    socket: UdpSocket,
    run_for: Duration,
    ping_interval: Duration,
    mut on_quote: impl FnMut(&StockQuote),
) -> io::Result<()> {
    socket.set_read_timeout(Some(UDP_READ_TIMEOUT))?;

    let server_addr: ServerAddrCell = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));

    let pinger_socket = socket.try_clone()?;
    let ping_handle = ping::spawn_udp_ping_loop(
        pinger_socket,
        Arc::clone(&server_addr),
        ping_interval,
        Arc::clone(&stop),
    );

    let mut buf = vec![0u8; UDP_RECV_BUF];
    let deadline = Instant::now() + run_for;

    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                let Ok(text) = std::str::from_utf8(&buf[..n]) else {
                    continue;
                };
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                match Command::parse(text) {
                    Command::Pong | Command::Ping => continue,
                    _ => {}
                }
                if let Some(q) = StockQuote::from_json_line(text) {
                    if let Ok(mut g) = server_addr.lock() {
                        let _ = g.get_or_insert(src);
                    }
                    on_quote(&q);
                }
            }
            Err(ref e) if crate::net::is_udp_recv_timeout_or_wouldblock(e) => continue,
            Err(e) => {
                stop.store(true, Ordering::SeqCst);
                let _ = ping_handle.join();
                return Err(e);
            }
        }
    }

    stop.store(true, Ordering::SeqCst);
    let _ = ping_handle.join();
    Ok(())
}

/// Приём до сигнала `stop` (в т.ч. обработчик Ctrl+C): фоновый `PING`, общий `AtomicBool`.
pub fn receive_quotes_with_ping_until_stop(
    socket: UdpSocket,
    ping_interval: Duration,
    stop: Arc<AtomicBool>,
    mut on_quote: impl FnMut(&StockQuote),
) -> io::Result<()> {
    socket.set_read_timeout(Some(UDP_READ_TIMEOUT))?;

    let server_addr: ServerAddrCell = Arc::new(Mutex::new(None));
    let pinger_socket = socket.try_clone()?;
    let ping_handle = ping::spawn_udp_ping_loop(
        pinger_socket,
        Arc::clone(&server_addr),
        ping_interval,
        Arc::clone(&stop),
    );

    let mut buf = vec![0u8; UDP_RECV_BUF];
    while !stop.load(Ordering::SeqCst) {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                let Ok(text) = std::str::from_utf8(&buf[..n]) else {
                    continue;
                };
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                match Command::parse(text) {
                    Command::Pong | Command::Ping => continue,
                    _ => {}
                }
                if let Some(q) = StockQuote::from_json_line(text) {
                    if let Ok(mut g) = server_addr.lock() {
                        let _ = g.get_or_insert(src);
                    }
                    on_quote(&q);
                }
            }
            Err(ref e) if crate::net::is_udp_recv_timeout_or_wouldblock(e) => continue,
            Err(e) => {
                stop.store(true, Ordering::SeqCst);
                let _ = ping_handle.join();
                return Err(e);
            }
        }
    }

    let _ = ping_handle.join();
    Ok(())
}
