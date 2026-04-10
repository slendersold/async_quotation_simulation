use std::path::PathBuf;

use clap::{Parser, Subcommand};
use utils::client::run::{start_stream_client, start_tcp_ping_client};
use utils::protocol::DEFAULT_PING_INTERVAL_SECS;

fn init_logging() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .try_init();
}

#[derive(Parser)]
#[command(name = "client", about = "Quote stream subscriber (TCP + UDP)")]
struct ClientCli {
    #[command(subcommand)]
    command: ClientCmd,
}

#[derive(Subcommand)]
enum ClientCmd {
    /// Subscribe via STREAM, receive UDP quotes, send keep-alive PING
    Stream {
        /// Quote server TCP address (host:port)
        #[arg(long = "server-addr", short = 't', default_value = "127.0.0.1:9876")]
        server_addr: String,
        /// Local UDP port for incoming quotes
        #[arg(long = "udp-port", short = 'u', default_value_t = 34254)]
        udp_port: u16,
        /// Tickers file (one ticker per line)
        #[arg(long = "tickers-file", short = 'f')]
        tickers_file: PathBuf,
        /// Local bind address for UDP
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// UDP PING interval (seconds)
        #[arg(long, default_value_t = DEFAULT_PING_INTERVAL_SECS)]
        ping_interval_secs: u64,
        /// Stop after N seconds (omit to run until Ctrl+C)
        #[arg(long)]
        duration_secs: Option<u64>,
    },
    /// Send TCP PING and expect PONG (command channel check)
    TcpPing {
        #[arg(long = "server-addr", short = 't', default_value = "127.0.0.1:9876")]
        server_addr: String,
    },
}

fn main() {
    init_logging();
    let cli = ClientCli::parse();
    let result = match cli.command {
        ClientCmd::Stream {
            server_addr,
            udp_port,
            tickers_file,
            bind,
            ping_interval_secs,
            duration_secs,
        } => start_stream_client(
            &server_addr,
            &bind,
            udp_port,
            &tickers_file,
            ping_interval_secs,
            duration_secs,
        ),
        ClientCmd::TcpPing { server_addr } => start_tcp_ping_client(&server_addr),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
