use ruiden::Ruiden;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut ruiden = Ruiden::new("COM9", 250_000, 0x01)?;
    ruiden.fetch_all().await?;
    println!("{ruiden:#?}");

    Ok(())
}
