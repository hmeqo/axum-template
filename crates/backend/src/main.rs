use backend::{Result, cli};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    cli::run().await?;

    Ok(())
}
