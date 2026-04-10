//! Интеграционные тесты бинарников `server` и `client`.

use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use utils::model::StockQuote;
use utils::net;

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

    let stream_help = Command::new(env!("CARGO_BIN_EXE_client"))
        .args(["stream", "--help"])
        .output()
        .expect("client stream --help");
    assert!(stream_help.status.success(), "{:?}", stream_help.stderr);
    let sh = String::from_utf8_lossy(&stream_help.stdout);
    assert!(
        sh.contains("server-addr") || sh.contains("server_addr"),
        "{}",
        sh
    );
    assert!(sh.contains("udp-port") || sh.contains("udp_port"), "{}", sh);
    assert!(sh.contains("tickers-file") || sh.contains("tickers_file"), "{}", sh);
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
fn cli_server_returns_err_for_unknown_ticker_stream() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--listen", "127.0.0.1:0", "--seed", "42"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");
    let stdout = child.stdout.take().expect("server stdout");
    let tcp = net::read_ready_listen_addr_from_read(stdout).expect("READY line");
    let _guard = KillChild(child);

    let mut stream = TcpStream::connect(tcp).expect("tcp connect");
    stream
        .write_all(b"STREAM udp://127.0.0.1:49999 ZZZ_UNKNOWN_TICKER_FOR_ERR_TEST\n")
        .expect("write STREAM");
    stream.flush().ok();

    let mut reader = BufReader::new(stream);
    let line = net::read_command_line(&mut reader, net::MAX_COMMAND_LINE_BYTES)
        .expect("read response")
        .expect("non-empty response");
    let t = line.trim();
    assert!(
        t.to_ascii_uppercase().starts_with("ERR"),
        "expected ERR … response, got: {t:?}"
    );
}

#[test]
fn cli_tcp_ping_with_running_server() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--listen", "127.0.0.1:0", "--seed", "1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn server");
    let stdout = child.stdout.take().expect("server stdout");
    let tcp = net::read_ready_listen_addr_from_read(stdout).expect("READY line");
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
    let tcp = net::read_ready_listen_addr_from_read(stdout).expect("READY line");
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
        .filter_map(|line| StockQuote::from_json_line(line.trim()))
        .collect();

    assert!(!quotes.is_empty(), "no quotes on stdout");
    assert!(quotes.iter().any(|q| q.ticker == "AAPL"));
    assert!(quotes.iter().any(|q| q.ticker == "TSLA"));
    for q in &quotes {
        assert!(q.ticker == "AAPL" || q.ticker == "TSLA");
    }
}
