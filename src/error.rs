//! Единственный файл с типами ошибок библиотеки: IO, протокол, сеть, парсинг.

use std::error::Error as StdError;
use std::fmt;
use std::net::AddrParseError;
use std::num::{ParseFloatError, ParseIntError};
use std::str::Utf8Error;
use std::{io, sync::mpsc};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    AddrParse(AddrParseError),
    Utf8(Utf8Error),
    ParseInt(ParseIntError),
    ParseFloat(ParseFloatError),
    ChannelSend,
    ChannelRecv,
    Timeout,
    Shutdown,
    Protocol(ProtocolError),
    Command(CommandError),
    Quote(QuoteError),
    Tickers(TickersError),
    Server(ServerError),
    Client(ClientError),
    InvalidData(&'static str),
}

#[derive(Debug)]
pub enum ProtocolError {
    EmptyCommand,
    UnknownCommand(String),
    InvalidFormat(&'static str),
    MissingField(&'static str),
    InvalidAddress(String),
}

#[derive(Debug)]
pub enum CommandError {
    EmptyTickers,
    InvalidTicker(String),
    DuplicateTickers,
    InvalidUri(String),
}

#[derive(Debug)]
pub enum QuoteError {
    InvalidQuote(&'static str),
    MissingField(&'static str),
    InvalidPrice,
    InvalidVolume,
    InvalidTimestamp,
}

#[derive(Debug)]
pub enum TickersError {
    FileEmpty,
    InvalidLine(String),
    TooManyTickers,
}

#[derive(Debug)]
pub enum ServerError {
    TcpBindFailed(String),
    TcpAcceptFailed,
    UdpBindFailed(String),
    UdpSendFailed,
    UdpRecvFailed,
    KeepAliveTimeout,
    ClientNotFound,
}

#[derive(Debug)]
pub enum ClientError {
    TcpConnectFailed(String),
    TcpWriteFailed,
    UdpBindFailed(String),
    UdpRecvFailed,
    PingFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::AddrParse(e) => write!(f, "address parse error: {e}"),
            Error::Utf8(e) => write!(f, "utf-8 error: {e}"),
            Error::ParseInt(e) => write!(f, "integer parse error: {e}"),
            Error::ParseFloat(e) => write!(f, "float parse error: {e}"),
            Error::ChannelSend => write!(f, "channel send failed"),
            Error::ChannelRecv => write!(f, "channel receive failed"),
            Error::Timeout => write!(f, "operation timed out"),
            Error::Shutdown => write!(f, "shutdown requested"),
            Error::Protocol(e) => write!(f, "protocol error: {e}"),
            Error::Command(e) => write!(f, "command error: {e}"),
            Error::Quote(e) => write!(f, "quote error: {e}"),
            Error::Tickers(e) => write!(f, "tickers error: {e}"),
            Error::Server(e) => write!(f, "server error: {e}"),
            Error::Client(e) => write!(f, "client error: {e}"),
            Error::InvalidData(msg) => write!(f, "invalid data: {msg}"),
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::EmptyCommand => write!(f, "empty command"),
            ProtocolError::UnknownCommand(cmd) => write!(f, "unknown command: {cmd}"),
            ProtocolError::InvalidFormat(msg) => write!(f, "invalid format: {msg}"),
            ProtocolError::MissingField(field) => write!(f, "missing field: {field}"),
            ProtocolError::InvalidAddress(addr) => write!(f, "invalid address: {addr}"),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::EmptyTickers => write!(f, "empty tickers list"),
            CommandError::InvalidTicker(t) => write!(f, "invalid ticker: {t}"),
            CommandError::DuplicateTickers => write!(f, "duplicate tickers"),
            CommandError::InvalidUri(uri) => write!(f, "invalid uri: {uri}"),
        }
    }
}

impl fmt::Display for QuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuoteError::InvalidQuote(msg) => write!(f, "invalid quote: {msg}"),
            QuoteError::MissingField(field) => write!(f, "missing field: {field}"),
            QuoteError::InvalidPrice => write!(f, "invalid price"),
            QuoteError::InvalidVolume => write!(f, "invalid volume"),
            QuoteError::InvalidTimestamp => write!(f, "invalid timestamp"),
        }
    }
}

impl fmt::Display for TickersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TickersError::FileEmpty => write!(f, "tickers file is empty"),
            TickersError::InvalidLine(line) => write!(f, "invalid ticker line: {line}"),
            TickersError::TooManyTickers => write!(f, "too many tickers"),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::TcpBindFailed(addr) => write!(f, "tcp bind failed: {addr}"),
            ServerError::TcpAcceptFailed => write!(f, "tcp accept failed"),
            ServerError::UdpBindFailed(addr) => write!(f, "udp bind failed: {addr}"),
            ServerError::UdpSendFailed => write!(f, "udp send failed"),
            ServerError::UdpRecvFailed => write!(f, "udp receive failed"),
            ServerError::KeepAliveTimeout => write!(f, "keep-alive timeout"),
            ServerError::ClientNotFound => write!(f, "client not found"),
        }
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::TcpConnectFailed(addr) => write!(f, "tcp connect failed: {addr}"),
            ClientError::TcpWriteFailed => write!(f, "tcp write failed"),
            ClientError::UdpBindFailed(addr) => write!(f, "udp bind failed: {addr}"),
            ClientError::UdpRecvFailed => write!(f, "udp receive failed"),
            ClientError::PingFailed => write!(f, "ping failed"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::AddrParse(e) => Some(e),
            Error::Utf8(e) => Some(e),
            Error::ParseInt(e) => Some(e),
            Error::ParseFloat(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::Io(value)
    }
}

impl From<AddrParseError> for Error {
    fn from(value: AddrParseError) -> Self {
        Error::AddrParse(value)
    }
}

impl From<Utf8Error> for Error {
    fn from(value: Utf8Error) -> Self {
        Error::Utf8(value)
    }
}

impl From<ParseIntError> for Error {
    fn from(value: ParseIntError) -> Self {
        Error::ParseInt(value)
    }
}

impl From<ParseFloatError> for Error {
    fn from(value: ParseFloatError) -> Self {
        Error::ParseFloat(value)
    }
}

impl<T> From<mpsc::SendError<T>> for Error {
    fn from(_: mpsc::SendError<T>) -> Self {
        Error::ChannelSend
    }
}

impl From<mpsc::RecvError> for Error {
    fn from(_: mpsc::RecvError) -> Self {
        Error::ChannelRecv
    }
}
