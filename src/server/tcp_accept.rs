//! Принятие TCP-подключений и обмен командами протокола (канал команд к серверу).
//!
//! Использует [`crate::net`] и [`crate::protocol::Command`].

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::error::{Error, ProtocolError, ServerError};
use crate::net;
use crate::protocol::Command;

/// Слушать TCP для входящих команд (обёртка над [`net::tcp_listen`] с маппингом в [`ServerError`]).
pub fn bind(addr: impl ToSocketAddrs) -> crate::Result<TcpListener> {
    net::tcp_listen(addr).map_err(|e| Error::Server(ServerError::TcpBindFailed(e.to_string())))
}

/// Принять следующее соединение.
pub fn accept(listener: &TcpListener) -> crate::Result<(TcpStream, std::net::SocketAddr)> {
    listener.accept().map_err(Error::from)
}

/// Прочитать одну команду из буферизованного потока.
pub fn read_command(reader: &mut impl BufRead) -> crate::Result<Option<Command>> {
    Command::read_from(reader).map_err(Error::from)
}

/// Записать одну команду строкой с переводом строки ([`net::write_command_line`]).
pub fn write_command(writer: &mut impl Write, cmd: &Command) -> crate::Result<()> {
    net::write_command_line(writer, &cmd.to_string()).map_err(Error::from)
}

/// Прочитать из `stream` одну команду; если это [`Command::Ping`], ответить [`Command::Pong`].
///
/// Для чтения и записи используется [`TcpStream::try_clone`], чтобы не терять буфер `BufReader`.
pub fn reply_pong_to_ping(stream: &mut TcpStream) -> crate::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    match Command::read_from(&mut reader)? {
        None => Err(Error::Protocol(ProtocolError::EmptyCommand)),
        Some(Command::Ping) => {
            write_command(stream, &Command::Pong)?;
            Ok(())
        }
        Some(Command::Pong) => Err(Error::Protocol(ProtocolError::UnknownCommand(
            "unexpected PONG from peer".into(),
        ))),
        Some(Command::Stream { .. }) => Err(Error::Protocol(ProtocolError::InvalidFormat(
            "STREAM is handled elsewhere",
        ))),
        Some(Command::Unknown(s)) => Err(Error::Protocol(ProtocolError::UnknownCommand(s))),
    }
}
