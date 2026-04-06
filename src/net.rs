//! Сетевые утилиты: адреса, TCP для команд, UDP для стрима и keep-alive.
//!
//! Команды и их разбор — в [`crate::protocol`].
//!
//! # Как это работает в реальной схеме
//!
//! 1. **Канал команд (обычно TCP)**  
//!    Клиент подключается к серверу (например `connect("server:9876")`) и отправляет одну строку
//!    с переводом строки, например `STREAM udp://127.0.0.1:34254 AAPL,TSLA`. Сервер читает строку
//!    ([`read_command_line`]), парсит через [`crate::protocol::Command::parse`], при валидном
//!    `STREAM` заводит отдельный поток под этого клиента.
//!
//! 2. **Поток котировок (UDP)**  
//!    В потоке клиента сервер держит (или переиспользует) **UDP-сокет**, привязанный к своей стороне
//!    ([`udp_bind`]), и шлёт датаграммы на **`udp_addr` из команды** — тот адрес, где клиент
//!    заранее сделал `UdpSocket::bind` и слушает. Каждая датаграмма — например одна строка котировки
//!    ([`crate::model::StockQuote::to_bytes`] / текстовый формат по договорённости).
//!
//! 3. **Ping / Pong (UDP)**  
//!    По заданию клиент шлёт `PING` **на тот же UDP-сокет сервера**, с которого приходят котировки
//!    (на пару `IP:порт` источника датаграмм). Сервер в основном или фоновом потоке читает входящие
//!    UDP-сообщения ([`UdpSocket::recv_from`]); на `PING` отвечает `PONG` на `src` адрес клиента.
//!    Если за [`crate::protocol::DEFAULT_PING_TIMEOUT_SECS`] не было `PING`, стрим для этого клиента
//!    останавливают и поток завершают.
//!
//! 4. **Потоки и данные**  
//!    Один общий [`crate::server::generator::QuoteGenerator`] в отдельном потоке пишет котировки в
//!    канал; потоки-отправители подписаны на общий broadcast (своя обвязка над `mpsc` / `crossbeam`)
//!    и фильтруют тикеры под своего клиента перед `send_to`.

use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs, UdpSocket};

/// Порт по умолчанию для приёма текстовых команд по TCP (если конфиг не задаёт другой).
pub const DEFAULT_TCP_COMMAND_PORT: u16 = 9_876;

/// Максимальная длина одной строки команды (защита от мусора/DoS при чтении по TCP).
pub const MAX_COMMAND_LINE_BYTES: usize = 4 * 1024;

/// Разобрать `IP:port` или `[IPv6]:port` после `trim`.
pub fn parse_addr(s: &str) -> io::Result<SocketAddr> {
    s.trim()
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

/// Слушать TCP для входящих соединений с командами.
pub fn tcp_listen(addr: impl ToSocketAddrs) -> io::Result<TcpListener> {
    TcpListener::bind(addr)
}

/// Привязать UDP-сокет (отправка котировок и приём `PING` на одном порту у сервера).
pub fn udp_bind(addr: impl ToSocketAddrs) -> io::Result<UdpSocket> {
    UdpSocket::bind(addr)
}

/// Прочитать одну строку до `\n` (срезать `\r\n` / `\n`), не длиннее `max_len` байт с учётом `\n`.
pub fn read_command_line(reader: &mut impl BufRead, max_len: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let n = reader.read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > max_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "command line exceeds max_len",
        ));
    }
    while matches!(buf.last().copied(), Some(b'\n' | b'\r')) {
        buf.pop();
    }
    String::from_utf8(buf).map(Some).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, e.utf8_error().to_string())
    })
}

/// Записать строку команды с `\n` (удобно для тестов и простых клиентов).
pub fn write_command_line(writer: &mut impl Write, line: &str) -> io::Result<()> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Отправить UDP-датаграмму на `addr`; дозаписать остаток, если `send_to` отдал не всё.
pub fn udp_send_all(sock: &UdpSocket, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
    let mut sent = 0;
    while sent < buf.len() {
        let n = sock.send_to(&buf[sent..], addr)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "UDP send_to wrote 0 bytes",
            ));
        }
        sent += n;
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn parse_addr_trims() {
        assert_eq!(
            parse_addr("  127.0.0.1:80 \n").unwrap(),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 80))
        );
    }

    #[test]
    fn parse_addr_rejects_garbage() {
        assert!(parse_addr("not-an-addr").is_err());
    }

    #[test]
    fn read_command_line_strips_crlf() {
        let mut c = Cursor::new(b"STREAM udp://127.0.0.1:1 A,B\r\n");
        let line = read_command_line(&mut c, 256).unwrap().unwrap();
        assert_eq!(line, "STREAM udp://127.0.0.1:1 A,B");
    }

    #[test]
    fn read_command_line_eof_empty() {
        let mut c = Cursor::new(b"");
        assert!(read_command_line(&mut c, 256).unwrap().is_none());
    }

    #[test]
    fn read_command_line_too_long() {
        let mut c = Cursor::new(vec![b'a'; MAX_COMMAND_LINE_BYTES + 10]);
        assert!(read_command_line(&mut c, MAX_COMMAND_LINE_BYTES).is_err());
    }

    #[test]
    fn udp_bind_loopback_ephemeral() {
        let s = udp_bind("127.0.0.1:0").unwrap();
        let _ = s.local_addr().unwrap();
    }
}
