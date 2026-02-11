//! CLI argument parsing.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "executor")]
#[command(about = "Dev PM Agent executor — desktop daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the executor daemon (WebSocket client, Cursor CLI) [default]
    Run,

    /// Generate and register a device key for first-run setup (before account exists)
    BootstrapDevice,

    /// Register a new webapp controller device
    RegisterDevice {
        /// Word-style registration code from webapp keygen
        #[arg(value_name = "CODE")]
        code: String,

        /// Admin password
        #[arg(value_name = "PASSWORD")]
        password: String,
    },
}
