//! Текстовый протокол команд между клиентом и сервером (стриминг котировок по UDP, keep-alive).
//!
//! Чтение строки с TCP и разбор адреса в `STREAM` согласованы с [`crate::net`] ([`crate::net::read_command_line`], [`crate::net::parse_addr`], [`crate::net::MAX_COMMAND_LINE_BYTES`]).
//!
//! Формат команды STREAM (от клиента серверу):
//! ```text
//! STREAM udp://127.0.0.1:12345 AAPL,TSLA
//! ```
//! Формат Ping (от клиента серверу, по UDP):
//! ```text
//! PING
//! ```
//! Формат Pong (от сервера клиенту, по UDP):
//! ```text
//! PONG
//! ```

use crate::net;
use std::io::{self, BufRead};
use std::net::SocketAddr;

/// Представление команд протокола стриминга.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Запустить UDP-стрим котировок для определённых тикеров на адрес.
    /// `STREAM udp://ip:port TICKER1,TICKER2,...`
    Stream {
        udp_addr: SocketAddr,
        tickers: Vec<String>,
    },
    /// Проверка связи (keep-alive, по UDP)
    Ping,
    /// Ответ на Ping (по UDP)
    Pong,
    /// Неизвестная или некорректная команда
    Unknown(String),
}

impl Command {
    /// Читает одну строку команды из потока (как после [`net::read_command_line`]) и парсит её.
    pub fn read_from(reader: &mut impl BufRead) -> io::Result<Option<Self>> {
        match net::read_command_line(reader, net::MAX_COMMAND_LINE_BYTES)? {
            None => Ok(None),
            Some(line) => Ok(Some(Self::parse(&line))),
        }
    }

    /// Парсит строку командного протокола в [`Command`].
    ///
    /// Строки длиннее [`net::MAX_COMMAND_LINE_BYTES`] байт считаются некорректными (как при чтении по TCP).
    pub fn parse(command_str: &str) -> Self {
        if command_str.len() > net::MAX_COMMAND_LINE_BYTES {
            return Command::Unknown("command line exceeds MAX_COMMAND_LINE_BYTES".to_string());
        }
        let trimmed = command_str.trim();
        if trimmed.eq_ignore_ascii_case("PING") {
            return Command::Ping;
        }
        if trimmed.eq_ignore_ascii_case("PONG") {
            return Command::Pong;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() >= 3 && tokens[0].eq_ignore_ascii_case("STREAM") {
            let addr_str = tokens[1];
            let ticker_list = tokens[2..].join(" ");
            let udp_prefix = "udp://";
            if let Some(addr_str) = addr_str.strip_prefix(udp_prefix) {
                if let Ok(socket_addr) = net::parse_addr(addr_str) {
                    let tickers = ticker_list
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    return Command::Stream {
                        udp_addr: socket_addr,
                        tickers,
                    };
                }
            }
        }
        Command::Unknown(trimmed.to_string())
    }

    /// Форматирует команду в строку (отправка клиенту или логи).
    pub fn to_string(&self) -> String {
        match self {
            Command::Ping => "PING".to_string(),
            Command::Pong => "PONG".to_string(),
            Command::Stream { udp_addr, tickers } => {
                format!("STREAM udp://{} {}", udp_addr, tickers.join(","))
            }
            Command::Unknown(s) => s.clone(),
        }
    }
}

/// Таймаут ожидания Ping от клиента перед остановкой стрима (секунды).
pub const DEFAULT_PING_TIMEOUT_SECS: u64 = 5;
pub const PING_COMMAND: &str = "PING";
pub const PONG_COMMAND: &str = "PONG";

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_from_reads_ping_line() {
        let mut c = Cursor::new(b"PING\n");
        assert_eq!(
            Command::read_from(&mut c).unwrap(),
            Some(Command::Ping)
        );
    }

    #[test]
    fn read_from_eof_none() {
        let mut c = Cursor::new(b"");
        assert_eq!(Command::read_from(&mut c).unwrap(), None);
    }

    #[test]
    fn parse_rejects_oversized_line() {
        let huge = "x".repeat(net::MAX_COMMAND_LINE_BYTES + 1);
        assert!(matches!(
            Command::parse(&huge),
            Command::Unknown(msg) if msg.contains("MAX_COMMAND_LINE_BYTES")
        ));
    }

    #[test]
    fn parse_stream_command() {
        let cmd = "STREAM udp://127.0.0.1:34254 AAPL,TSLA";
        let parsed = Command::parse(cmd);
        assert_eq!(
            parsed,
            Command::Stream {
                udp_addr: "127.0.0.1:34254".parse().unwrap(),
                tickers: vec!["AAPL".to_string(), "TSLA".to_string()]
            }
        );
    }

    #[test]
    fn parse_ping_and_pong() {
        assert_eq!(Command::parse("PING"), Command::Ping);
        assert_eq!(Command::parse("PONG"), Command::Pong);
    }

    #[test]
    fn unknown_command_fallback() {
        let cmd = "HELLO WORLD";
        assert_eq!(
            Command::parse(cmd),
            Command::Unknown("HELLO WORLD".to_string())
        );
    }

    #[test]
    fn tolerant_to_spaces_and_casing() {
        let cmd = "  StReAm  udp://1.2.3.4:5678   GOOG  ";
        let parsed = Command::parse(cmd);
        assert_eq!(
            parsed,
            Command::Stream {
                udp_addr: "1.2.3.4:5678".parse().unwrap(),
                tickers: vec!["GOOG".to_string()],
            }
        );
    }

    #[test]
    fn stream_command_to_string_roundtrip() {
        let cmd = Command::Stream {
            udp_addr: "10.0.0.25:12345".parse().unwrap(),
            tickers: vec!["AAPL".to_string(), "TSLA".to_string()],
        };
        let s = cmd.to_string();
        assert_eq!(s, "STREAM udp://10.0.0.25:12345 AAPL,TSLA");
    }
}
