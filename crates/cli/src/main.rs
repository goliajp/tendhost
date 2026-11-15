use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tendhost_cli::api::Api;
use tendhost_cli::commands::{self, Command};
use tendhost_cli::config::{Config, default_path};

/// The command line for a tendhost server.
#[derive(Parser)]
#[command(name = "tendhost", version)]
struct Cli {
    /// Configuration file [default: ~/.config/tendhost/cli.toml]
    #[arg(long, global = true, env = "TENDHOST_CONFIG")]
    config: Option<PathBuf>,
    /// Print the server's json instead of a table
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Top,
}

#[derive(Subcommand)]
enum Top {
    /// Remember a server and an api token for the commands that follow
    Login {
        /// The server's address, e.g. https://tend.example.com
        #[arg(long)]
        server: String,
        /// An api token created in the dashboard
        #[arg(long, env = "TENDHOST_TOKEN")]
        token: String,
    },
    #[command(flatten)]
    Command(Command),
}

#[tokio::main]
async fn main() -> ExitCode {
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tendhost: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn execute(cli: Cli) -> Result<()> {
    let path = match cli.config {
        Some(path) => path,
        None => default_path(std::env::var("HOME").ok().as_deref())?,
    };
    match cli.command {
        Top::Login { server, token } => {
            let api = Api::new(&server, &token);
            let me = api
                .get("/session")
                .await
                .context("the server did not accept this token")?;
            Config {
                server: server.trim_end_matches('/').to_string(),
                token,
            }
            .save(&path)?;
            println!(
                "Signed in to {} as {} ({}).",
                api.server(),
                me["name"].as_str().unwrap_or_default(),
                me["role"].as_str().unwrap_or_default()
            );
            Ok(())
        }
        Top::Command(command) => {
            let config = Config::load(&path)?;
            let mut out = String::new();
            commands::run(
                &Api::new(&config.server, &config.token),
                command,
                cli.json,
                &mut out,
            )
            .await?;
            print!("{out}");
            Ok(())
        }
    }
}
