use std::path::PathBuf;

use clap::{Parser, Subcommand};
use utils::client::run::{start_stream_client, start_tcp_ping_client};

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
        /// Quote server TCP address
        #[arg(long, short = 't', default_value = "127.0.0.1:9876")]
        tcp_server: String,
        /// Local UDP port for incoming quotes
        #[arg(long, short = 'u', default_value_t = 34254)]
        udp_port: u16,
        /// Tickers file (one ticker per line)
        #[arg(long, short = 'f')]
        tickers_file: PathBuf,
        /// Local bind address for UDP
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// UDP PING interval (seconds)
        #[arg(long, default_value_t = 2)]
        ping_interval_secs: u64,
        /// Stop after N seconds (omit to run until Ctrl+C)
        #[arg(long)]
        duration_secs: Option<u64>,
    },
    /// Send TCP PING and expect PONG (command channel check)
    TcpPing {
        #[arg(long, short = 't', default_value = "127.0.0.1:9876")]
        tcp_server: String,
    },
}

fn main() {
    let cli = ClientCli::parse();
    let result = match cli.command {
        ClientCmd::Stream {
            tcp_server,
            udp_port,
            tickers_file,
            bind,
            ping_interval_secs,
            duration_secs,
        } => start_stream_client(
            &tcp_server,
            &bind,
            udp_port,
            &tickers_file,
            ping_interval_secs,
            duration_secs,
        ),
        ClientCmd::TcpPing { tcp_server } => start_tcp_ping_client(&tcp_server),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
