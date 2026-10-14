#![cfg_attr(not(test), warn(clippy::unwrap_used))]

use axum_template::{Result, cli};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    cli::run().await?;

    Ok(())
}
