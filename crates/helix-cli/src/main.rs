use std::{collections::HashSet, fs, path::PathBuf};

use clap::{Parser, Subcommand};
use helix_db::{auth::ApiKey, config::HelixConfig, server::HelixServer};
use tokio::runtime::Runtime;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about = "HelixDB command line interface")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the HelixDB server
    Serve {
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    /// Initialize configuration on disk
    Init,
    /// Validate configuration without starting the server
    Check {
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    /// Deploy configuration to an environment
    Push {
        #[arg(default_value = "dev")]
        environment: String,
    },
    /// Manage API keys
    ApiKey {
        #[command(subcommand)]
        command: ApiKeyCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ApiKeyCommand {
    /// Generate a new API key and print the plaintext value
    Create {
        #[arg(short, long)]
        name: String,
        #[arg(short, long, value_delimiter = ',')]
        scopes: Vec<String>,
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { config } => {
            let cfg = HelixConfig::load(config)?;
            let server = HelixServer::new(cfg)?;
            let rt = Runtime::new()?;
            rt.block_on(async move { server.run().await })?;
        }
        Command::Init => {
            let path = default_config_path();
            if path.exists() {
                println!("Configuration already exists at {}", path.display());
            } else {
                let cfg = HelixConfig::default();
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                let serialized = serde_yaml::to_string(&cfg)?;
                fs::write(&path, serialized)?;
                println!("Initialized configuration at {}", path.display());
            }
        }
        Command::Check { config } => {
            HelixConfig::load(config)?;
            println!("Configuration OK");
        }
        Command::Push { environment } => {
            println!("Pushing configuration to {environment}");
            println!("(simulated) helix push {environment}");
        }
        Command::ApiKey { command } => match command {
            ApiKeyCommand::Create {
                name,
                scopes,
                config,
            } => {
                let mut cfg = HelixConfig::load(config.clone())?;
                let scopes_set: HashSet<String> = if scopes.is_empty() {
                    HashSet::new()
                } else {
                    scopes.into_iter().collect()
                };
                let (api_key, raw) = ApiKey::generate_secure(&name, scopes_set);
                cfg.security.api_keys.push(helix_db::config::ApiKeyConfig {
                    name: name.clone(),
                    key: api_key.hashed_key.clone(),
                    scopes: api_key.scopes.iter().cloned().collect(),
                });
                let target = config.unwrap_or_else(default_config_path);
                let serialized = serde_yaml::to_string(&cfg)?;
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(&target, serialized)?;
                println!("Created API key {name}. Store this secret securely:\n{raw}");
            }
        },
    }
    Ok(())
}

fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .map(|dir| dir.join(".helix").join("braindb-config.yaml"))
        .unwrap_or_else(|| PathBuf::from("./braindb-config.yaml"))
}
