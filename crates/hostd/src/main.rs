use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tendhost_hostd::client::Client;
use tendhost_hostd::config::{Config, default_path};
use tendhost_hostd::run::Removed;
use tendhost_hostd::{collect, run};
use tendhost_protocol::Mode;

/// The tendhost daemon for a managed host.
#[derive(Parser)]
#[command(name = "tendhostd", version)]
struct Cli {
    /// Configuration file [default: /etc/tendhost/hostd.toml for root,
    /// ~/.config/tendhost/hostd.toml otherwise]
    #[arg(long, global = true, env = "TENDHOSTD_CONFIG")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Join a tendhost server with a one-time enrollment token
    Enroll {
        /// The server's address, e.g. https://tend.example.com
        #[arg(long)]
        server: String,
        /// The enrollment token the dashboard showed
        #[arg(long, env = "TENDHOST_ENROLL_TOKEN")]
        token: String,
        /// The name to appear under [default: this machine's hostname]
        #[arg(long)]
        name: Option<String>,
        /// Also let the server run commands on this host. Without it the
        /// host only reports
        #[arg(long)]
        manage: bool,
    },
    /// Report to the server until stopped
    Run,
}

/// sysexits.h EX_CONFIG: a service manager should not restart on it
const EXIT_NOT_ENROLLED: u8 = 78;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tendhostd: {error:#}");
            if error.is::<Removed>() {
                ExitCode::from(EXIT_NOT_ENROLLED)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

async fn execute(cli: Cli) -> Result<()> {
    let path = match cli.config {
        Some(path) => path,
        None => default_path(is_root(), std::env::var("HOME").ok().as_deref())?,
    };
    match cli.command {
        Command::Enroll {
            server,
            token,
            name,
            manage,
        } => {
            let mode = if manage { Mode::Manage } else { Mode::Observe };
            let name = match name {
                Some(name) => name,
                None => collect::hostname()?,
            };
            let enrolled = Client::new(&server)?
                .enroll(&token, &name, mode, collect::facts()?)
                .await?;
            let config = Config {
                server: server.trim_end_matches('/').to_string(),
                host_id: enrolled.host_id,
                credential: enrolled.credential,
                mode,
            };
            config.save(&path)?;
            println!(
                "Enrolled as {} ({}). Configuration written to {}.",
                name,
                config.host_id,
                path.display()
            );
            println!("Start reporting with: tendhostd run");
            Ok(())
        }
        Command::Run => run::run(Config::load(&path)?).await,
    }
}

/// Whether the daemon runs as root, which decides where its
/// configuration lives.
fn is_root() -> bool {
    std::env::var("USER").is_ok_and(|user| user == "root")
        || std::env::var("HOME").is_ok_and(|home| home == "/root" || home == "/var/root")
}
