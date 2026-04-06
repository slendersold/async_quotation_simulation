//! Логика бинарника `client`: подписка STREAM, приём UDP, TCP-ping.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::client::{tcp_command, udp_recv};
use crate::error::Error;
use crate::net;
use crate::protocol::Command;
use crate::server::tickers;

/// Подключение по TCP, `STREAM`, приём котировок на UDP с фоновым `PING`.
pub fn start_stream_client(
    tcp_server: &str,
    udp_bind_host: &str,
    udp_port: u16,
    tickers_path: &Path,
    ping_interval_secs: u64,
    duration_secs: Option<u64>,
) -> crate::Result<()> {
    let list = tickers::load_tickers_from_path(tickers_path).map_err(Error::from)?;
    let udp_addr: std::net::SocketAddr = format!("{udp_bind_host}:{udp_port}")
        .parse()
        .map_err(Error::from)?;
    let sock = net::udp_bind(udp_addr)?;
    let mut tcp = tcp_command::connect(tcp_server)?;
    tcp_command::send_command(
        &mut tcp,
        &Command::Stream {
            udp_addr,
            tickers: list,
        },
    )?;
    drop(tcp);

    let ping_interval = Duration::from_secs(ping_interval_secs.max(1));

    match duration_secs {
        Some(secs) => {
            udp_recv::receive_quotes_with_ping_on_socket_with_cb(
                sock,
                Duration::from_secs(secs),
                ping_interval,
                |q| println!("{}", q.to_string()),
            )
            .map_err(Error::from)?;
        }
        None => {
            let stop = Arc::new(AtomicBool::new(false));
            let s = stop.clone();
            ctrlc::set_handler(move || {
                s.store(true, Ordering::SeqCst);
            })
            .map_err(|e| {
                Error::Io(io::Error::new(
                    io::ErrorKind::Other,
                    format!("ctrlc handler: {e}"),
                ))
            })?;
            udp_recv::receive_quotes_with_ping_until_stop(sock, ping_interval, stop, |q| {
                println!("{}", q.to_string());
            })
            .map_err(Error::from)?;
        }
    }
    Ok(())
}

/// Одна проверка TCP: `PING` / `PONG`.
pub fn start_tcp_ping_client(tcp_server: &str) -> crate::Result<()> {
    let mut tcp = tcp_command::connect(tcp_server)?;
    tcp_command::send_ping_expect_pong(&mut tcp)
}
