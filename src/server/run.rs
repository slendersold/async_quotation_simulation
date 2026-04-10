//! Запуск TCP-сервера команд и хаба котировок.

use std::collections::HashSet;
use std::io::{self, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{info, warn};
use crate::net;
use crate::protocol::{format_err_line, Command, RESPONSE_OK_LINE};
use super::registry::QuoteHub;
use super::streaming;
use super::tcp_accept;
use super::tickers;

/// Пауза при пустом `accept`, чтобы периодически проверять флаг остановки (Ctrl+C).
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Слушать TCP, вывести в stdout строку `READY <addr>`, обрабатывать `STREAM` и TCP-`PING`.
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
    // Контракт запуска: одна строка с адресом прослушивания TCP.
    println!("READY {addr}");
    io::stdout().flush().map_err(crate::Error::from)?;
    info!("TCP command server listening on {addr}");

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
            info!("stop signal (Ctrl+C), exiting accept loop");
            break;
        }
        match listener.accept() {
            Ok((stream, peer)) => {
                let hub = hub.clone();
                thread::spawn(move || {
                    if let Err(e) = handle_tcp_client(stream, hub) {
                        warn!("peer {peer}: {e}");
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

fn validate_stream_command(tickers: &[String]) -> Result<(), String> {
    if tickers.is_empty() {
        return Err("empty tickers list".to_string());
    }
    let unique: HashSet<&str> = tickers.iter().map(String::as_str).collect();
    if unique.len() != tickers.len() {
        return Err("duplicate tickers".to_string());
    }
    let allowed: HashSet<String> = tickers::all_default()
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for t in tickers {
        if !allowed.contains(t) {
            return Err(format!("unknown ticker: {t}"));
        }
    }
    Ok(())
}

fn handle_tcp_client(mut stream: TcpStream, hub: QuoteHub) -> crate::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    while let Some(cmd) = tcp_accept::read_command(&mut reader)? {
        match cmd {
            Command::Stream { udp_addr, tickers } => {
                match validate_stream_command(&tickers) {
                    Ok(()) => {
                        net::write_command_line(&mut stream, RESPONSE_OK_LINE)?;
                        let rx = hub.subscribe(tickers);
                        let stop = Arc::new(AtomicBool::new(false));
                        let _ = streaming::spawn_udp_stream_worker(udp_addr, rx, stop);
                    }
                    Err(msg) => {
                        net::write_command_line(&mut stream, &format_err_line(&msg))?;
                    }
                }
            }
            Command::Ping => {
                tcp_accept::write_command(&mut stream, &Command::Pong)?;
            }
            Command::Pong => {}
            Command::Unknown(raw) => {
                let msg = if raw.len() > 200 {
                    format!("unknown command: {}…", &raw[..200])
                } else {
                    format!("unknown command: {raw}")
                };
                net::write_command_line(&mut stream, &format_err_line(&msg))?;
            }
        }
    }
    Ok(())
}
