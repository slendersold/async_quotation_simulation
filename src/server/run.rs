//! Запуск TCP-сервера команд и хаба котировок (для бинарника `server`).

use std::io::{self, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::protocol::Command;
use super::registry::QuoteHub;
use super::streaming;
use super::tcp_accept;

/// Пауза при пустом `accept`, чтобы периодически проверять флаг остановки (Ctrl+C).
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Слушать TCP, печатать в stdout строку `READY <addr>` (для тестов), обрабатывать `STREAM` и TCP-`PING`.
pub fn start_tcp_command_server(
    listen: &str,
    emit_interval_ms: u64,
    seed: Option<u64>,
) -> crate::Result<()> {
    let seed = seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1)
    });
    let interval = Duration::from_millis(emit_interval_ms.max(1));
    let hub = QuoteHub::spawn_generator_thread(seed, interval);
    let listener = tcp_accept::bind(listen)?;
    let addr = listener.local_addr().map_err(crate::Error::from)?;
    println!("READY {addr}");
    io::stdout().flush().map_err(crate::Error::from)?;

    listener.set_nonblocking(true).map_err(crate::Error::from)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    ctrlc::set_handler(move || {
        stop_flag.store(true, Ordering::SeqCst);
    })
    .map_err(|e| {
        crate::Error::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("ctrlc handler: {e}"),
        ))
    })?;

    loop {
        if stop.load(Ordering::SeqCst) {
            eprintln!("server: stop signal (Ctrl+C), exiting accept loop");
            break;
        }
        match listener.accept() {
            Ok((stream, peer)) => {
                let hub = hub.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_tcp_client(stream, hub) {
                        eprintln!("peer {peer}: {e}");
                    }
                });
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

fn handle_tcp_client(mut stream: TcpStream, hub: QuoteHub) -> crate::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    while let Some(cmd) = tcp_accept::read_command(&mut reader)? {
        match cmd {
            Command::Stream { udp_addr, tickers } => {
                let rx = hub.subscribe(tickers);
                let stop = Arc::new(AtomicBool::new(false));
                let _ = streaming::spawn_udp_stream_worker(udp_addr, rx, stop);
            }
            Command::Ping => {
                tcp_accept::write_command(&mut stream, &Command::Pong)?;
            }
            Command::Pong | Command::Unknown(_) => {}
        }
    }
    Ok(())
}
