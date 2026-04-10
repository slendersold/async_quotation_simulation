//! TCP-подключение к серверу и отправка текстовых команд протокола.
//!
//! Использует [`crate::net`] и [`crate::protocol::Command`] (аналогично серверному [`crate::server::tcp_accept`]).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};

use crate::error::{ClientError, Error, ProtocolError};
use crate::net;
use crate::protocol::Command;

/// Подключиться к серверу команд.
pub fn connect(addr: impl ToSocketAddrs) -> crate::Result<TcpStream> {
    TcpStream::connect(addr).map_err(|e| {
        Error::Client(ClientError::TcpConnectFailed(e.to_string()))
    })
}

/// Отправить одну команду (строка + `\n`).
pub fn send_command(writer: &mut impl Write, cmd: &Command) -> crate::Result<()> {
    net::write_command_line(writer, &cmd.to_string()).map_err(Error::from)
}

/// Отправить [`Command::Stream`] и прочитать ответ сервера: `OK` или `ERR …`.
pub fn send_stream_expect_response(stream: &mut TcpStream, cmd: &Command) -> crate::Result<()> {
    let Command::Stream { .. } = cmd else {
        return Err(Error::Protocol(ProtocolError::InvalidFormat(
            "expected Command::Stream",
        )));
    };
    send_command(stream, cmd)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    match net::read_command_line(&mut reader, net::MAX_COMMAND_LINE_BYTES)? {
        None => Err(Error::Protocol(ProtocolError::EmptyCommand)),
        Some(line) => {
            crate::protocol::parse_stream_response_line(&line).map_err(|msg| {
                Error::Protocol(ProtocolError::UnknownCommand(format!("STREAM rejected: {msg}")))
            })
        }
    }
}

/// Прочитать одну команду.
pub fn read_command(reader: &mut impl BufRead) -> crate::Result<Option<Command>> {
    Command::read_from(reader).map_err(Error::from)
}

/// Отправить [`Command::Ping`] и прочитать ответ; ожидается [`Command::Pong`].
pub fn send_ping_expect_pong(stream: &mut TcpStream) -> crate::Result<()> {
    send_command(stream, &Command::Ping)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    match Command::read_from(&mut reader)? {
        None => Err(Error::Protocol(ProtocolError::EmptyCommand)),
        Some(Command::Pong) => Ok(()),
        Some(other) => Err(Error::Protocol(ProtocolError::UnknownCommand(other.to_string()))),
    }
}
