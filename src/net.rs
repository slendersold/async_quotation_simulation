//! Сетевые утилиты: адреса, TCP для команд, UDP для стрима и keep-alive.
//!
//! Разбор команд — в [`crate::protocol`].
//!
//! # Схема обмена
//!
//! 1. Канал команд (TCP): строка с `\n`, например `STREAM udp://127.0.0.1:34254 AAPL,TSLA`.
//!    Чтение — [`read_command_line`], разбор — [`crate::protocol::Command::parse`]. После валидного
//!    `STREAM` создаётся поток отправки UDP для этого подключения.
//!
//! 2. Поток котировок (UDP): сервер привязывает сокет [`udp_bind`], отправляет датаграммы на адрес
//!    из команды; клиент заранее делает `bind` на этом адресе. Одна датаграмма — JSON-строка
//!    [`crate::model::StockQuote::to_json_line`].
//!
//! 3. Keep-alive (UDP): клиент шлёт `PING` на адрес источника котировок; сервер отвечает `PONG`.
//!    При отсутствии `PING` дольше [`crate::protocol::DEFAULT_PING_TIMEOUT_SECS`] стрим останавливается.
//!
//! 4. Данные: [`crate::server::generator::QuoteGenerator`] в отдельном потоке формирует батчи;
//!    рассылка подписчикам через каналы и фильтрацию по тикерам перед `send_to`.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, ToSocketAddrs, UdpSocket};

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

/// Разбор строки старта сервера `READY <addr>` после [`read_command_line`].
///
/// Для `0.0.0.0:<port>` возвращает `127.0.0.1:<port>` (loopback для TCP-клиента на той же машине).
pub fn read_ready_listen_addr(reader: &mut impl BufRead) -> io::Result<SocketAddr> {
    let Some(line) = read_command_line(reader, MAX_COMMAND_LINE_BYTES)? else {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "missing READY line",
        ));
    };
    let rest = line.trim().strip_prefix("READY ").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "expected line starting with READY ",
        )
    })?;
    let addr: SocketAddr = rest.parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid socket address in READY line: {e}"),
        )
    })?;
    Ok(match addr.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port())
        }
        _ => addr,
    })
}

/// Читает `READY` из потока (например `ChildStdout` сервера).
pub fn read_ready_listen_addr_from_read(reader: impl Read) -> io::Result<SocketAddr> {
    read_ready_listen_addr(&mut BufReader::new(reader))
}

/// Ошибка `recv`/`recv_from` на UDP с таймаутом: повторить цикл ожидания.
pub fn is_udp_recv_timeout_or_wouldblock(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// Записать строку команды с `\n` и сбросить буфер.
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

    #[test]
    fn read_ready_listen_addr_parses_line() {
        let mut c = Cursor::new(b"READY 127.0.0.1:9876\n");
        let a = read_ready_listen_addr(&mut c).unwrap();
        assert_eq!(a, "127.0.0.1:9876".parse().unwrap());
    }

    #[test]
    fn read_ready_listen_addr_maps_unspecified_v4() {
        let mut c = Cursor::new(b"READY 0.0.0.0:5555\r\n");
        let a = read_ready_listen_addr(&mut c).unwrap();
        assert_eq!(a, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5555)));
    }
}
