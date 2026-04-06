//! Интеграционные тесты бинарников `server` и `client` (clap, STREAM, tcp-ping).

use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use utils::model::StockQuote;

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

struct KillChild(std::process::Child);
impl Drop for KillChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn cli_server_help() {
    let out = Command::new(env!("CARGO_BIN_EXE_server"))
        .arg("--help")
        .output()
        .expect("server --help");
    assert!(out.status.success(), "{:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("listen"), "{}", s);
}

#[test]
fn cli_client_help() {
    let out = Command::new(env!("CARGO_BIN_EXE_client"))
        .arg("--help")
        .output()
        .expect("client --help");
    assert!(out.status.success(), "{:?}", out.stderr);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("stream"), "{}", s);
    assert!(s.contains("tcp-ping") || s.contains("tcp_ping"), "{}", s);
}

#[test]
fn cli_client_stream_fails_on_missing_tickers_file() {
    let out = Command::new(env!("CARGO_BIN_EXE_client"))
        .args([
            "stream",
            "-t",
            "127.0.0.1:1",
            "-u",
            "9",
            "-f",
            "/nonexistent/tickers_cli_test.txt",
        ])
        .output()
        .expect("client stream");
    assert!(!out.status.success());
}

#[test]
fn cli_tcp_ping_with_running_server() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--listen", "127.0.0.1:0", "--seed", "1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn server");
    let stdout = child.stdout.take().expect("server stdout");
    let tcp = read_server_ready(stdout);
    let _guard = KillChild(child);

    let out = Command::new(env!("CARGO_BIN_EXE_client"))
        .args(["tcp-ping", "-t", &tcp.to_string()])
        .output()
        .expect("client tcp-ping");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn cli_stream_udp_127_0_0_1_34254_aapl_tsla_one_second() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args([
            "--listen",
            "127.0.0.1:0",
            "--emit-interval-ms",
            "25",
            "--seed",
            "4157438",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn server");
    let stdout = child.stdout.take().expect("server stdout");
    let tcp = read_server_ready(stdout);
    let _guard = KillChild(child);

    let tick_path: PathBuf = std::env::temp_dir().join(format!("cli_tickers_{}.txt", std::process::id()));
    std::fs::write(&tick_path, "AAPL\nTSLA\n").expect("write tickers");

    let output = Command::new(env!("CARGO_BIN_EXE_client"))
        .args([
            "stream",
            "-t",
            &tcp.to_string(),
            "-u",
            "34254",
            "-f",
            tick_path.to_str().unwrap(),
            "--duration-secs",
            "1",
            "--ping-interval-secs",
            "1",
            "--bind",
            "127.0.0.1",
        ])
        .output()
        .expect("client stream");

    let _ = std::fs::remove_file(&tick_path);

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let quotes: Vec<StockQuote> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| StockQuote::from_string(line.trim()))
        .collect();

    assert!(!quotes.is_empty(), "no quotes on stdout");
    assert!(quotes.iter().any(|q| q.ticker == "AAPL"));
    assert!(quotes.iter().any(|q| q.ticker == "TSLA"));
    for q in &quotes {
        assert!(q.ticker == "AAPL" || q.ticker == "TSLA");
    }
}
