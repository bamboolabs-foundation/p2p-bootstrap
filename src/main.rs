#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub use anyhow::{anyhow as custom_error, Result};
pub use zsn_logging::{debug, error, info, warn};

pub(crate) mod behaviours;
pub(crate) mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    zsn_logging::init();

    let cli::CLIArgs {
        port,
        secret_key: keypair,
    } = <cli::CLIArgs as clap::Parser>::parse();

    behaviours::SwarmService::create(keypair, port)?.run().await
}
