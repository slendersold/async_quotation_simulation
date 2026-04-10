//! Команда `STREAM` на фиксированный UDP-адрес, подписка AAPL/TSLA, keep-alive PING.

use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use utils::client::{tcp_command, udp_recv};
use utils::net;
use utils::protocol::{Command, RESPONSE_OK_LINE};
use utils::server::registry::QuoteHub;
use utils::server::streaming;
use utils::server::tcp_accept;

const UDP_PORT: u16 = 34_254;

#[test]
fn integration_stream_udp_127_0_0_1_34254_aapl_tsla_one_second() {
    let run = Duration::from_secs(1);
    let emit_interval = Duration::from_millis(25);
    let ping_interval = Duration::from_millis(350);

    let hub = QuoteHub::spawn_generator_thread(0xA17E, emit_interval);
    let listener = tcp_accept::bind("127.0.0.1:0").expect("tcp bind");
    let tcp_addr = listener.local_addr().expect("tcp local addr");

    let stop = Arc::new(AtomicBool::new(false));
    let stream_handle_slot: Arc<Mutex<Option<thread::JoinHandle<()>>>> =
        Arc::new(Mutex::new(None));

    let hub_srv = hub.clone();
    let stop_srv = stop.clone();
    let slot = stream_handle_slot.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = tcp_accept::accept(&listener).expect("accept");
        let mut reader = BufReader::new(&mut stream);
        let cmd = tcp_accept::read_command(&mut reader)
            .expect("read command")
            .expect("non-empty command");
        let Command::Stream { udp_addr, tickers } = cmd else {
            panic!("expected STREAM, got {cmd:?}");
        };
        assert_eq!(udp_addr.port(), UDP_PORT);
        assert!(tickers.contains(&"AAPL".to_string()));
        assert!(tickers.contains(&"TSLA".to_string()));
        net::write_command_line(&mut stream, RESPONSE_OK_LINE).expect("write OK");
        let rx = hub_srv.subscribe(tickers);
        let h = streaming::spawn_udp_stream_worker(udp_addr, rx, stop_srv);
        *slot.lock().unwrap() = Some(h);
    });

    let udp_addr = SocketAddr::from(([127, 0, 0, 1], UDP_PORT));
    let client = thread::spawn(move || {
        let sock = net::udp_bind(udp_addr).expect("bind client UDP before STREAM");
        let mut tcp = tcp_command::connect(tcp_addr).expect("tcp connect");
        tcp_command::send_stream_expect_response(
            &mut tcp,
            &Command::Stream {
                udp_addr,
                tickers: vec!["AAPL".into(), "TSLA".into()],
            },
        )
        .expect("send STREAM and OK");
        drop(tcp);
        udp_recv::receive_quotes_with_ping_on_socket(sock, run, ping_interval)
            .expect("UDP recv + ping")
    });

    server.join().expect("server thread panicked");
    let quotes = client.join().expect("client thread panicked");

    stop.store(true, Ordering::SeqCst);
    if let Some(h) = stream_handle_slot.lock().unwrap().take() {
        let _ = h.join();
    }

    assert!(
        !quotes.is_empty(),
        "expected at least one quote within 1 second"
    );
    for q in &quotes {
        assert!(
            q.ticker == "AAPL" || q.ticker == "TSLA",
            "unexpected ticker {}",
            q.ticker
        );
    }
    assert!(
        quotes.iter().any(|q| q.ticker == "AAPL"),
        "expected AAPL quotes"
    );
    assert!(
        quotes.iter().any(|q| q.ticker == "TSLA"),
        "expected TSLA quotes"
    );
}
