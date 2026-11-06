use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use pagelet::AppState;
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let state = AppState::open(&cli.data_dir)?;
    match cli.command {
        Command::Init => {
            if state.require_initialized().is_ok() {
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
        Command::Serve { listen } => pagelet::serve(state, &listen).await?,
    }
    Ok(())
}
