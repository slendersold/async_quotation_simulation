//! Один TCP/UDP-сервер, пять параллельных клиентов с разными списками тикеров.
//!
//! Случайная задержка перед подключением; длительность приёма случайна. Завершение процесса клиента:
//! либо по `--duration-secs`, либо через `Child::kill()`. Сценарий выполняется несколько раз подряд
//! с новым процессом сервера.
//!
//! После завершения всех клиентов: пауза 6 с, затем приём на бывших UDP-портах в течение фиксированного
//! окна — ожидается отсутствие датаграмм (серверный таймаут отсутствия PING — 5 с).

use std::io::{BufRead, BufReader, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use utils::model::StockQuote;

const SCENARIO_REPEATS: u32 = 5;
const N_CLIENTS: usize = 5;
/// Пауза после всех клиентов: дольше серверного таймаута отсутствия PING (5 с).
const SILENCE_AFTER_ALL_CLIENTS_SECS: u64 = 6;
/// Длительность пассивного `recv` на каждом бывшем UDP-порту клиента.
const PASSIVE_UDP_LISTEN_WINDOW: Duration = Duration::from_secs(2);

fn client_udp_port(iteration: u32, client_idx: usize) -> u16 {
    36_000 + (iteration as u16) * 40 + client_idx as u16 * 7
}

/// Проверка: на бывших портах клиентов за окно не приходит UDP-датаграмм.
fn assert_udp_silence_on_client_ports(iteration: u32) {
    thread::sleep(Duration::from_secs(SILENCE_AFTER_ALL_CLIENTS_SECS));

    let listeners: Vec<_> = (0..N_CLIENTS)
        .map(|client_idx| {
            let port = client_udp_port(iteration, client_idx);
            thread::spawn(move || {
                let bind_addr = format!("127.0.0.1:{port}");
                let sock = UdpSocket::bind(&bind_addr).unwrap_or_else(|e| {
                    panic!("iter {iteration} passive bind {bind_addr}: {e}");
                });
                let _ = sock.set_read_timeout(Some(Duration::from_millis(200)));
                let mut buf = [0u8; 2048];
                let deadline = Instant::now() + PASSIVE_UDP_LISTEN_WINDOW;
                while Instant::now() < deadline {
                    match sock.recv_from(&mut buf) {
                        Ok((n, src)) => {
                            return Some(format!(
                                "port {port}: got {n} bytes from {src}: {:?}",
                                &buf[..n.min(64)]
                            ));
                        }
                        Err(e)
                            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                        Err(e) => {
                            return Some(format!("port {port}: recv error: {e}"));
                        }
                    }
                }
                None
            })
        })
        .collect();

    for (client_idx, h) in listeners.into_iter().enumerate() {
        let violation = h.join().expect("passive UDP listener thread panicked");
        assert!(
            violation.is_none(),
            "iter {iteration} client {client_idx}: server still sent UDP after quiet period: {:?}",
            violation
        );
    }
}

/// Подмножества тикеров из `tickers.txt` сервера.
const CLIENT_TICKERS: [&[&str]; N_CLIENTS] = [
    &["AAPL", "MSFT"],
    &["GOOGL", "AMZN"],
    &["NVDA", "META"],
    &["TSLA", "JPM"],
    &["JNJ", "V"],
];

fn read_server_ready(stdout: impl std::io::Read) -> SocketAddr {
    let mut br = BufReader::new(stdout);
    let mut line = String::new();
    br.read_line(&mut line).expect("READY line");
    let rest = line.trim().strip_prefix("READY ").expect("READY prefix");
    let addr: SocketAddr = rest.parse().expect("socket addr");
    match addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port())
        }
        _ => addr,
    }
}

fn tickers_file_content(tickers: &[&str]) -> String {
    tickers.join("\n") + "\n"
}

fn parse_quotes_stdout(stdout: &[u8]) -> Vec<StockQuote> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| StockQuote::from_json_line(line.trim()))
        .collect()
}

fn allowed_ticker_set(client_idx: usize) -> std::collections::HashSet<String> {
    CLIENT_TICKERS[client_idx]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

#[test]
fn five_clients_random_timing_polite_or_kill_five_iterations() {
    let client_exe: &'static str = env!("CARGO_BIN_EXE_client");
    let server_exe: &'static str = env!("CARGO_BIN_EXE_server");
    let pid = std::process::id();

    for iteration in 0..SCENARIO_REPEATS {
        let mut rng = StdRng::seed_from_u64(0x5EED_0000_0000 ^ u64::from(iteration).wrapping_mul(0x9E37_79B9_7F4A_7C15));

        let mut server = Command::new(server_exe)
            .args([
                "--listen",
                "127.0.0.1:0",
                "--emit-interval-ms",
                "15",
                "--seed",
                &format!("{}", 9_001_u64 + u64::from(iteration)),
            ])
            .env("RUST_LOG", "off")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn server");

        let stdout = server.stdout.take().expect("server stdout");
        let tcp_addr = read_server_ready(stdout);
        let tcp_str = tcp_addr.to_string();

        let handles: Vec<_> = (0..N_CLIENTS)
            .map(|client_idx| {
                let tcp_str = tcp_str.clone();
                let client_exe = client_exe.to_string();
                let thread_seed = rng.next_u64();
                let mut rng = StdRng::seed_from_u64(thread_seed);

                thread::spawn(move || {
                    let connect_delay_ms = rng.gen_range(0_u64..180);
                    thread::sleep(Duration::from_millis(connect_delay_ms));

                    let tick_path = std::env::temp_dir().join(format!(
                        "multi_cli_{pid}_iter{iteration}_c{client_idx}.txt"
                    ));
                    std::fs::write(
                        &tick_path,
                        tickers_file_content(CLIENT_TICKERS[client_idx]),
                    )
                    .expect("write tickers file");

                    let udp_port = client_udp_port(iteration, client_idx);

                    let polite_exit = rng.gen_bool(0.55);
                    let allowed = allowed_ticker_set(client_idx);

                    if polite_exit {
                        // Завершение по истечении `--duration-secs`.
                        let duration_secs = rng.gen_range(2_u64..=5);
                        let out = Command::new(&client_exe)
                            .args([
                                "stream",
                                "-t",
                                &tcp_str,
                                "-u",
                                &udp_port.to_string(),
                                "-f",
                                tick_path.to_str().expect("utf8 temp path"),
                                "--bind",
                                "127.0.0.1",
                                "--duration-secs",
                                &duration_secs.to_string(),
                                "--ping-interval-secs",
                                "1",
                            ])
                            .env("RUST_LOG", "off")
                            .output()
                            .expect("client stream polite");

                        let _ = std::fs::remove_file(&tick_path);

                        assert!(
                            out.status.success(),
                            "iter {iteration} client {client_idx} polite exit failed: stderr={}",
                            String::from_utf8_lossy(&out.stderr)
                        );

                        let quotes = parse_quotes_stdout(&out.stdout);
                        assert!(
                            !quotes.is_empty(),
                            "iter {iteration} client {client_idx}: expected some quotes (duration {duration_secs}s)"
                        );
                        for q in &quotes {
                            assert!(
                                allowed.contains(&q.ticker),
                                "iter {iteration} client {client_idx}: ticker {} not in subscription {:?}",
                                q.ticker,
                                allowed
                            );
                        }
                    } else {
                        // Принудительное завершение: `Child::kill()`.
                        let mut child = Command::new(&client_exe)
                            .args([
                                "stream",
                                "-t",
                                &tcp_str,
                                "-u",
                                &udp_port.to_string(),
                                "-f",
                                tick_path.to_str().expect("utf8 temp path"),
                                "--bind",
                                "127.0.0.1",
                                "--ping-interval-secs",
                                "1",
                            ])
                            .env("RUST_LOG", "off")
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
                            .expect("client stream abrupt spawn");

                        let listen_ms = rng.gen_range(250_u64..900);
                        thread::sleep(Duration::from_millis(listen_ms));

                        let _ = child.kill();
                        let status = child.wait().expect("wait after kill");
                        assert!(
                            !status.success(),
                            "iter {iteration} client {client_idx}: expected non-zero status after kill"
                        );

                        let _ = std::fs::remove_file(&tick_path);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("client thread should not panic");
        }

        assert_udp_silence_on_client_ports(iteration);

        let _ = server.kill();
        let _ = server.wait();
    }
}
