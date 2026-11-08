use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use pagelet::AppState;
use std::{
    env,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about = "Self-hosted Markdown publishing")]
struct Cli {
    #[arg(long, env = "PAGELET_DATA_DIR", default_value = "data", global = true)]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init,
    RecoverAdmin,
    Serve {
        #[arg(long, env = "PAGELET_LISTEN", default_value = "127.0.0.1:8080")]
        listen: String,
        #[arg(long, env = "PAGELET_EXTERNAL_URL")]
        external_url: Option<String>,
        #[arg(long, env = "PAGELET_TRUSTED_PROXIES", value_delimiter = ',')]
        trusted_proxy: Vec<String>,
    },
}

fn read_input(prompt: &str, hidden: bool) -> Result<String> {
    if hidden && io::stdin().is_terminal() {
        return Ok(rpassword::prompt_password(prompt)?);
    }
    eprint!("{prompt}");
    io::stderr().flush()?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        bail!("Input ended unexpectedly");
    }
    if input.ends_with('\n') {
        input.pop();
        if input.ends_with('\r') {
            input.pop();
        }
    }
    Ok(input)
}

fn init_logging() -> Result<()> {
    const DEFAULT_FILTER: &str = "pagelet=info";
    let configured = match std::env::var("RUST_LOG") {
        Ok(value) if !value.trim().is_empty() => EnvFilter::try_new(value).map_err(|_| ()),
        Err(std::env::VarError::NotUnicode(_)) => Err(()),
        _ => Ok(EnvFilter::new(DEFAULT_FILTER)),
    };
    let invalid = configured.is_err();
    tracing_subscriber::fmt()
        .with_env_filter(configured.unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER)))
        .with_writer(io::stderr)
        .with_ansi(
            io::stderr().is_terminal()
                && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()),
        )
        .try_init()
        .map_err(anyhow::Error::msg)?;
    if invalid {
        tracing::warn!(event = %"invalid_log_filter", "Invalid RUST_LOG. Using pagelet=info.");
    }
    Ok(())
}

fn bootstrap_administrator(state: &AppState) -> Result<()> {
    if state.is_initialized()? {
        return Ok(());
    }
    const USERNAME: &str = "PAGELET_ADMIN_USERNAME";
    const PASSWORD: &str = "PAGELET_ADMIN_PASSWORD";
    let (username, password) = match (env::var_os(USERNAME), env::var_os(PASSWORD)) {
        (None, None) => return Ok(()),
        (None, Some(_)) => bail!("{USERNAME} is required when {PASSWORD} is set"),
        (Some(_), None) => bail!("{PASSWORD} is required when {USERNAME} is set"),
        (Some(username), Some(password)) => (username, password),
    };
    let username = username
        .into_string()
        .map_err(|_| anyhow::anyhow!("{USERNAME} must be valid UTF-8"))?;
    let password = password
        .into_string()
        .map_err(|_| anyhow::anyhow!("{PASSWORD} must be valid UTF-8"))?;
    state.initialize(&username, &password)?;
    tracing::info!(event = %"administrator_initialized", source = %"environment");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let state = AppState::open(&cli.data_dir)?;
    match cli.command {
        Command::Init => {
            if state.is_initialized()? {
                bail!("Instance is already initialized");
            }
            let username = read_input("Username: ", false)?;
            let password = read_input("Password: ", true)?;
            let confirmation = read_input("Confirm password: ", true)?;
            if password != confirmation {
                bail!("Passwords do not match");
            }
            let token = state.initialize(&username, &password)?;
            println!("API token: {token}");
            println!("Save this token now; it cannot be retrieved later.");
        }
        Command::RecoverAdmin => {
            let username = read_input("Administrator username: ", false)?;
            let password = read_input("New password: ", true)?;
            let confirmation = read_input("Confirm password: ", true)?;
            if password != confirmation {
                bail!("Passwords do not match");
            }
            let token = state.recover_administrator(&username, &password)?;
            println!("API token: {token}");
            println!("Save this token now; it cannot be retrieved later.");
        }
        Command::Serve {
            listen,
            external_url,
            trusted_proxy,
        } => {
            init_logging()?;
            let mut state = state;
            state.deployment = pagelet::Deployment::new(
                external_url.as_deref(),
                trusted_proxy.iter().map(String::as_str),
            )?;
            bootstrap_administrator(&state)?;
            pagelet::serve(state, &listen).await?;
        }
    }
    Ok(())
}
