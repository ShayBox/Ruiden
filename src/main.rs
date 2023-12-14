use anyhow::Result;
use ruiden::Ruiden;

// TODO: Add a command line interface with clap

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut ruiden = Ruiden::new("COM9", 250_000, 0x01)?;
    ruiden.fetch_info().await?;
    println!("{:#?}", ruiden.info);

    Ok(())
}
