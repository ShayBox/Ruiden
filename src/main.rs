use anyhow::Result;
use clap::Parser;
use ruiden::Ruiden;

#[derive(Debug, Parser)]
struct Args {
    /// Serial Port Path
    #[arg(short, long)]
    path: String,

    /// Serial Baud Rate
    #[arg(short, long, default_value_t = 115_200)]
    baud_rate: u32,

    /// Modbus Slave ID
    #[arg(short, long, default_value_t = 1)]
    slave_id: u8,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut ruiden = Ruiden::new(args.path, args.baud_rate, args.slave_id)?;
    ruiden.fetch_info().await?;
    println!("{:#?}", ruiden.info);

    Ok(())
}
