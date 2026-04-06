//! Интеграция: TCP-сервер принимает PING и отвечает PONG; клиент отправляет PING и получает PONG.

use std::thread;

use utils::client::tcp_command;
use utils::server::tcp_accept;

#[test]
fn integration_tcp_client_server_ping_pong() {
    let listener = tcp_accept::bind("127.0.0.1:0").expect("bind command listener");
    let addr = listener.local_addr().expect("local addr");

    let server = thread::spawn(move || {
        let (mut stream, _) = tcp_accept::accept(&listener).expect("accept");
        tcp_accept::reply_pong_to_ping(&mut stream).expect("reply PONG to PING");
    });

    let mut client = tcp_command::connect(addr).expect("connect");
    tcp_command::send_ping_expect_pong(&mut client).expect("ping / pong roundtrip");

    server.join().expect("server thread panicked");
}
