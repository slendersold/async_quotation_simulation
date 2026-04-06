use clap::Parser;
use utils::server::run::start_tcp_command_server;

#[derive(Parser)]
#[command(name = "server", about = "Quote generator: TCP commands + UDP streaming")]
struct ServerCli {
    /// TCP listen address (commands: STREAM, PING, …)
    #[arg(long, default_value = "0.0.0.0:9876")]
    listen: String,
    /// Milliseconds between quote batches from the generator
    #[arg(long, default_value_t = 1u64)]
    emit_interval_ms: u64,
    /// RNG seed for quotes (default: wall-clock based)
    #[arg(long)]
    seed: Option<u64>,
}

fn main() {
    let cli = ServerCli::parse();
    if let Err(e) = start_tcp_command_server(&cli.listen, cli.emit_interval_ms, cli.seed) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
