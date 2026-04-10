//! Поток UDP-стриминга: приём отфильтрованных пакетов из канала, отправка на клиента, ответ на `PING`.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use log::warn;
use crate::model::StockQuote;
use crate::net;
use crate::protocol::{Command, DEFAULT_PING_TIMEOUT_SECS, PONG_COMMAND};

use super::keepalive;

const UDP_IO_BUF: usize = 2048;
/// Таймаут `recv` на канале котировок; совмещён с проверкой keep-alive в основном цикле.
const QUOTE_RECV_TIMEOUT: Duration = Duration::from_millis(100);
/// Таймаут `recv_from` для входящих UDP `PING`.
const PING_RECV_TIMEOUT: Duration = Duration::from_millis(250);

/// Запускает поток: шлёт котировки на `dest`, слушает `PING` на том же UDP-сокете, отвечает `PONG`.
///
/// Останавливается при `stop == true`, обрыве канала или истечении keep-alive с момента последнего `PING`.
pub fn spawn_udp_stream_worker(
    dest: SocketAddr,
    quote_rx: mpsc::Receiver<Vec<StockQuote>>,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let socket = match net::udp_bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                warn!("udp bind for stream to {dest}: {e}");
                return;
            }
        };
        let _ = socket.set_read_timeout(Some(PING_RECV_TIMEOUT));

        let last_ping = Arc::new(Mutex::new(Instant::now()));
        let ping_timeout = Duration::from_secs(DEFAULT_PING_TIMEOUT_SECS);

        let recv_sock = match socket.try_clone() {
            Ok(s) => s,
            Err(e) => {
                warn!("udp try_clone for ping recv: {e}");
                return;
            }
        };
        let last_ping_recv = Arc::clone(&last_ping);
        let stop_recv = Arc::clone(&stop);
        let _ping_thread = thread::spawn(move || {
            let mut buf = [0u8; UDP_IO_BUF];
            while !stop_recv.load(Ordering::SeqCst) {
                match recv_sock.recv_from(&mut buf) {
                    Ok((n, src)) => {
                        let Ok(text) = std::str::from_utf8(&buf[..n]) else {
                            continue;
                        };
                        match Command::parse(text.trim()) {
                            Command::Ping => {
                                if let Ok(mut lp) = last_ping_recv.lock() {
                                    *lp = Instant::now();
                                }
                                let _ = recv_sock.send_to(PONG_COMMAND.as_bytes(), src);
                            }
                            _ => {}
                        }
                    }
                    Err(ref e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            || e.kind() == io::ErrorKind::TimedOut => {}
                    Err(_) => break,
                }
            }
        });

        while !stop.load(Ordering::SeqCst) {
            let stale = match last_ping.lock() {
                Ok(lp) => keepalive::ping_deadline_exceeded(*lp, ping_timeout),
                Err(_) => true,
            };
            if stale {
                break;
            }

            match quote_rx.recv_timeout(QUOTE_RECV_TIMEOUT) {
                Ok(quotes) => {
                    for q in quotes {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        send_quote_datagram(&socket, dest, &q);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        stop.store(true, Ordering::SeqCst);
    })
}

fn send_quote_datagram(socket: &std::net::UdpSocket, dest: SocketAddr, q: &StockQuote) {
    let line = match q.to_json_line() {
        Ok(s) => s,
        Err(e) => {
            warn!("quote json encode: {e}");
            return;
        }
    };
    if let Err(e) = net::udp_send_all(socket, line.as_bytes(), dest) {
        warn!("udp send quote to {dest}: {e}");
    }
}
