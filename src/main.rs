use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use ruiden::{bootloader::Bootloader, register::Register, Ruiden};

#[derive(Debug, Parser)]
#[command(author, version, about = "Ruiden RD power supply Modbus tool")]
struct Args {
    /// Serial port
    #[arg(short, long, default_value = "/dev/ttyUSB0", global = true)]
    port: String,

    /// Serial baud rate
    #[arg(short, long, default_value_t = 115_200, global = true)]
    baud_rate: u32,

    /// Modbus slave ID
    #[arg(short, long, default_value_t = 1, global = true)]
    slave_id: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print device identification and live measurements
    Info,
    /// Print every Modbus register from 0 to 255
    Dump,
    /// Configure setpoints and options
    Set {
        /// Voltage setpoint in volts
        #[arg(long)]
        v_set:  Option<f32>,
        /// Current setpoint in amps
        #[arg(long)]
        i_set:  Option<f32>,
        /// Turn the output on or off
        #[arg(long, value_parser = ["on", "off"])]
        output: Option<String>,
        /// Recall a preset slot (0-9)
        #[arg(long)]
        preset: Option<u16>,
    },
    /// Reboot the unit into bootloader mode and optionally flash firmware
    Flash {
        /// Firmware image; when omitted only reboot and print device info
        firmware: Option<PathBuf>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let Args {
        port,
        baud_rate,
        slave_id,
        command,
    } = Args::parse();

    match command {
        Command::Info => info(&port, baud_rate, slave_id).await?,
        Command::Dump => dump(&port, baud_rate, slave_id).await?,
        Command::Set {
            v_set,
            i_set,
            output,
            preset,
        } => {
            set(&port, baud_rate, slave_id, v_set, i_set, output, preset).await?;
        }
        Command::Flash { firmware } => {
            flash(&port, baud_rate, slave_id, firmware.as_deref()).await?;
        }
    }

    Ok(())
}

async fn info(port: &str, baud_rate: u32, slave_id: u8) -> Result<()> {
    let mut psu = Ruiden::connect(port, baud_rate, slave_id).await?;
    psu.update().await?;

    let datetime = if psu.info.model.has_rtc() {
        psu.get_date_time().await?.to_string()
    } else {
        "n/a".to_owned()
    };

    println!("Model   : {} ({})", psu.info.model, psu.info.id);
    println!("SN      : {}", psu.info.sn);
    println!("FW      : {}.{}", psu.info.fw / 100, psu.info.fw % 100);
    println!("INT_C   : {} C", psu.info.int_c);
    println!("INT_F   : {} F", psu.info.int_f);
    println!("V_SET   : {}", psu.info.v_set);
    println!("I_SET   : {}", psu.info.i_set);
    println!("V_OUT   : {}", psu.info.v_out);
    println!("I_OUT   : {}", psu.info.i_out);
    println!("AH_DISP : {}", psu.info.ah);
    println!("P_OUT   : {}", psu.info.p_out);
    println!("V_IN    : {}", psu.info.v_in);
    println!("KEYPAD  : {}", psu.info.keypad);
    println!("OVP_OCP : {}", psu.info.ovp_ocp);
    println!("CV_CC   : {}", psu.info.cv_cc);
    println!("OUTPUT  : {}", psu.info.output);
    println!("PRESET  : {}", psu.info.preset);
    println!("I_RANGE : {}", psu.info.i_range);
    println!("BAT_MODE: {}", psu.status.bat_mode);
    println!("V_BAT   : {}", psu.status.v_bat);
    println!("EXT_C   : {} C", psu.status.ext_c);
    println!("EXT_F   : {} F", psu.status.ext_f);
    println!("AH      : {}", psu.status.ah);
    println!("WH      : {}", psu.status.wh);
    println!("DATETIME: {datetime}");
    println!("TAKE_OK : {}", psu.is_take_ok().await?);
    println!("TAKE_OUT: {}", psu.is_take_out().await?);
    println!("BOOT_POW: {}", psu.is_boot_pow().await?);
    println!("BUZZ    : {}", psu.is_buzz().await?);
    println!("LOGO    : {}", psu.is_logo().await?);
    println!("LANG    : {}", psu.get_lang().await?);
    println!("LIGHT   : {}", psu.get_light().await?);

    Ok(())
}

async fn dump(port: &str, baud_rate: u32, slave_id: u8) -> Result<()> {
    let mut psu = Ruiden::connect(port, baud_rate, slave_id).await?;

    for address in 0..=255 {
        let name = Register::try_from(address).map_or("", Register::name);
        match psu.read_words(address, 1).await {
            Ok(words) => println!("{name:>11} {address:03}: {}", words[0]),
            Err(err) => println!("{name:>11} {address:03}: ERROR ({err})"),
        }
    }

    Ok(())
}

async fn set(
    port: &str,
    baud_rate: u32,
    slave_id: u8,
    v_set: Option<f32>,
    i_set: Option<f32>,
    output: Option<String>,
    preset: Option<u16>,
) -> Result<()> {
    let mut psu = Ruiden::connect(port, baud_rate, slave_id).await?;

    if let Some(v_set) = v_set {
        psu.set_v_set(v_set).await?;
    }
    if let Some(i_set) = i_set {
        psu.set_i_set(i_set).await?;
    }
    if let Some(preset) = preset {
        psu.set_preset(preset).await?;
    }
    if let Some(output) = output.as_deref() {
        psu.set_output(output == "on").await?;
    }

    // Re-read so the printed values are what the device actually holds.
    psu.fetch_info().await?;
    println!("V_SET : {} V", psu.info.v_set);
    println!("I_SET : {} A", psu.info.i_set);
    println!("OUTPUT: {}", psu.info.output);
    println!("PRESET: {}", psu.info.preset);

    Ok(())
}

async fn flash(port: &str, baud_rate: u32, slave_id: u8, firmware: Option<&Path>) -> Result<()> {
    let mut boot = Bootloader::enter(port, baud_rate, slave_id).await?;
    let info = boot.info().await?;

    println!("Model   : {} ({})", info.model(), info.id);
    println!("FW      : {}.{}", info.fw / 100, info.fw % 100);
    println!("SN      : {}", info.sn);

    let Some(firmware) = firmware else {
        println!("Device is in bootloader mode; no firmware given, not flashing.");
        return Ok(());
    };

    let image = std::fs::read(firmware)?;
    println!("Flashing {} bytes...", image.len());
    boot.flash(&image, |_| print!(".")).await?;
    println!(" done.");

    Ok(())
}
