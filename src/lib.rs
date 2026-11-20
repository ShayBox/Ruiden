//! # Ruiden
//!
//! A Rust library (and companion CLI) for Ruiden RD-series bench power
//! supplies over Modbus RTU: RD6006, RD6006P, RD6012, RD6012P, RD6018,
//! RD6018W, RD6024, RD6030 and the battery-powered RK6006.
//!
//! ## Library example
//!
//! ```no_run
//! use anyhow::Result;
//! use ruiden::Ruiden;
//!
//! # #[allow(dead_code)]
//! async fn run() -> Result<()> {
//! //   Open the port and identify the unit (registers 0-20).
//! let mut psu = Ruiden::connect("/dev/ttyUSB0", 115_200, 1).await?;
//! println!("Found {}", psu.info.model);
//!
//! //   Refresh every measurement in two bulk reads.
//! psu.update().await?;
//! println!("{:.2} V / {:.3} A", psu.info.v_out, psu.info.i_out);
//!
//! //   Setpoints are scaled using the model detected by `connect`.
//! psu.set_v_set(4.20).await?;
//! psu.set_i_set(0.69).await?;
//! psu.set_output(true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Polling strategy
//!
//! Prefer [`Ruiden::fetch_info`] and [`Ruiden::fetch_status`] (or
//! [`Ruiden::update`]) over the single-register getters: each getter is its own
//! Modbus transaction, while the bulk reads fetch whole blocks in one.
//!
//! ## Safety notes
//!
//! - The calibration registers (55-62) and [`Register::CAL_COMMIT`] are
//!   exposed through the low-level read/write helpers but deliberately have no
//!   high-level API: miscalibration can make the supply dangerous.
//! - Some Ruiden stock firmware revisions break Modbus on USB; the community
//!   `UniSoft` firmware (see the README for links) is known to work.

pub mod bootloader;
pub mod info;
pub mod model;
pub mod register;

use anyhow::{bail, ensure, Context as _, Result};
use info::{double_word, per_mille, serial_number, temperature};
pub use info::{CvCc, DateTime, Info, Preset, Protection, Status};
pub use model::Model;
use register::Register;
pub use register::{CAL_COMMIT_NVRAM, CMD_REBOOT};
use tokio::time::{timeout, Duration};
use tokio_modbus::{client::Context, prelude::*, Address, Quantity};
use tokio_serial::SerialStream;

/// A single Modbus holding register value.
pub type Word = u16;
/// A vector of Modbus holding register values.
pub type Words = Vec<Word>;
/// Two consecutive register values, ordered `[high, low]`.
pub type WordPair = [Word; 2];

/// The address written to a boolean option register to enable it.
const BOOL_TRUE: Word = 1;
/// The address written to a boolean option register to disable it.
const BOOL_FALSE: Word = 0;

/// Number of preset slots (M0-M9).
pub const PRESET_SLOTS: u8 = 10;

/// A connected Ruiden power supply.
///
/// The [`Self::info`] and [`Self::status`] fields hold the most recently
/// fetched blocks, refreshed by [`Self::fetch_info`], [`Self::fetch_status`]
/// and [`Self::update`].
#[derive(Debug)]
pub struct Ruiden {
    ctx:        Context,
    pub info:   Info,
    pub status: Status,
}

impl Ruiden {
    /* Constructors */

    /// Opens the serial port without talking to the device.
    ///
    /// Call [`Self::fetch_info`] before using setpoints: scale factors come
    /// from the device model. [`Self::connect`] does this for you.
    ///
    /// # Errors
    /// Will return `Err` if the serial port cannot be opened.
    pub fn new(port: &str, baud_rate: u32, slave_id: SlaveId) -> Result<Self> {
        let builder = tokio_serial::new(port, baud_rate);
        let transport = SerialStream::open(&builder)?;
        let slave = Slave(slave_id);
        let ctx = rtu::attach_slave(transport, slave);

        Ok(Self {
            ctx,
            info: Info::default(),
            status: Status::default(),
        })
    }

    /// Opens the serial port and identifies the device in one step.
    ///
    /// # Errors
    /// Will return `Err` if the serial port cannot be opened or the device
    /// does not answer the identification read.
    pub async fn connect(port: &str, baud_rate: u32, slave_id: SlaveId) -> Result<Self> {
        let mut psu = Self::new(port, baud_rate, slave_id)?;
        psu.fetch_info().await?;
        Ok(psu)
    }

    /* Fetchers */

    /// Bulk-reads the identification/state block, registers 0-20.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails or the response is short.
    pub async fn fetch_info(&mut self) -> Result<&Info> {
        let address = Register::ID.address();
        let quantity = Register::I_RANGE.address() - address + 1;
        let words = self.read_words(address, quantity).await?;
        self.info = Info::try_from(words.as_slice())?;

        Ok(&self.info)
    }

    /// Bulk-reads the battery/energy block, registers 32-41.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails or the response is short.
    pub async fn fetch_status(&mut self) -> Result<&Status> {
        let address = Register::BAT_MODE.address();
        let quantity = Register::WH_L.address() - address + 1;
        let words = self.read_words(address, quantity).await?;
        self.status = Status::from_words(&words, self.info.model)?;

        Ok(&self.status)
    }

    /// Refreshes [`Self::info`] and [`Self::status`] in two bulk reads.
    ///
    /// # Errors
    /// Will return `Err` if either bulk read fails.
    pub async fn update(&mut self) -> Result<()> {
        self.fetch_info().await?;
        self.fetch_status().await?;

        Ok(())
    }

    /* Getters */

    /// Raw device ID, see [`Model::from_id`].
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_id(&mut self) -> Result<Word> {
        self.read_one(Register::ID).await
    }

    /// Zero-padded serial number.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_sn(&mut self) -> Result<String> {
        let pair = self.read_pair(Register::SN_H).await?;

        Ok(serial_number(double_word(pair)))
    }

    /// Firmware version, `136` = v1.36.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_fw(&mut self) -> Result<Word> {
        self.read_one(Register::FW).await
    }

    /// Internal temperature in °C.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_int_c(&mut self) -> Result<i16> {
        Ok(temperature(self.read_pair(Register::INT_C_S).await?))
    }

    /// Internal temperature in °F.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_int_f(&mut self) -> Result<i16> {
        Ok(temperature(self.read_pair(Register::INT_F_S).await?))
    }

    /// Voltage setpoint in volts.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_v_set(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::V_SET).await?) / self.info.model.v_mul())
    }

    /// Current setpoint in amps.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_i_set(&mut self) -> Result<f32> {
        let mul = self.info.model.i_mul(self.info.i_range);
        Ok(f32::from(self.read_one(Register::I_SET).await?) / mul)
    }

    /// Output voltage in volts.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_v_out(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::V_OUT).await?) / self.info.model.v_mul())
    }

    /// Output current in amps.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_i_out(&mut self) -> Result<f32> {
        let mul = self.info.model.i_mul(self.info.i_range);
        Ok(f32::from(self.read_one(Register::I_OUT).await?) / mul)
    }

    /// Output power in watts.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_p_out(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::P_OUT).await?) / self.info.model.p_mul())
    }

    /// Input voltage in volts.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_v_in(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::V_IN).await?) / self.info.model.v_in_mul())
    }

    /// Charge counter shown on the display in amp-hours (register 12).
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_ah_display(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::AH).await?) / 1_000.0)
    }

    /// Whether the keypad is locked.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_keypad_locked(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::KEYPAD).await? != 0)
    }

    /// Which output protection tripped, if any.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_protection(&mut self) -> Result<Protection> {
        Ok(Protection::from(self.read_one(Register::OVP_OCP).await?))
    }

    /// Regulation mode, constant voltage or constant current.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_cv_cc(&mut self) -> Result<CvCc> {
        Ok(CvCc::from(self.read_one(Register::CV_CC).await?))
    }

    /// Whether the output is enabled.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_output_on(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OUTPUT).await? != 0)
    }

    /// Active preset slot.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_preset(&mut self) -> Result<Word> {
        self.read_one(Register::PRESET).await
    }

    /// Current range (RD6012P only: `0` = 6A, `1` = 12A).
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_i_range(&mut self) -> Result<Word> {
        self.read_one(Register::I_RANGE).await
    }

    /// Whether battery mode is active.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_bat_mode(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::BAT_MODE).await? != 0)
    }

    /// Battery voltage in volts.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_v_bat(&mut self) -> Result<f32> {
        Ok(f32::from(self.read_one(Register::V_BAT).await?) / self.info.model.v_mul())
    }

    /// External temperature probe in °C; unconnected probes read `-128`.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_ext_c(&mut self) -> Result<i16> {
        Ok(temperature(self.read_pair(Register::EXT_C_S).await?))
    }

    /// External temperature probe in °F.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_ext_f(&mut self) -> Result<i16> {
        Ok(temperature(self.read_pair(Register::EXT_F_S).await?))
    }

    /// Amp-hour counter, registers 38/39 combined, divided by 1000.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_ah(&mut self) -> Result<f32> {
        let pair = self.read_pair(Register::AH_H).await?;

        Ok(per_mille(double_word(pair)))
    }

    /// Watt-hour counter, registers 40/41 combined, divided by 1000.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_wh(&mut self) -> Result<f32> {
        let pair = self.read_pair(Register::WH_H).await?;

        Ok(per_mille(double_word(pair)))
    }

    /// Real-time clock, registers 48-53.
    ///
    /// # Errors
    /// Will return `Err` if the model has no RTC or the Modbus read fails.
    pub async fn get_date_time(&mut self) -> Result<DateTime> {
        ensure!(
            self.info.model.has_rtc(),
            "{} has no real-time clock",
            self.info.model
        );
        let words = self.read_words(Register::YEAR.address(), 6).await?;
        let words: [Word; 6] = words
            .as_slice()
            .try_into()
            .context("short read for date/time block")?;

        Ok(DateTime::from_words(words))
    }

    /// Option: setpoints are accepted while the output is on.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_take_ok(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OPT_TAKE_OK).await? != 0)
    }

    /// Option: setpoints are recalled when the output turns off.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_take_out(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OPT_TAKE_OUT).await? != 0)
    }

    /// Option: the output state is restored on power-on.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_boot_pow(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OPT_BOOT_POW).await? != 0)
    }

    /// Option: buzzer enabled.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_buzz(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OPT_BUZZ).await? != 0)
    }

    /// Option: boot logo enabled.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn is_logo(&mut self) -> Result<bool> {
        Ok(self.read_one(Register::OPT_LOGO).await? != 0)
    }

    /// Option: display language index.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_lang(&mut self) -> Result<Word> {
        self.read_one(Register::OPT_LANG).await
    }

    /// Option: backlight level.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn get_light(&mut self) -> Result<Word> {
        self.read_one(Register::OPT_LIGHT).await
    }

    /// Contents of a preset/memory slot `0..=9` (registers 80-119).
    ///
    /// # Errors
    /// Will return `Err` if the slot is out of range or the read fails.
    pub async fn get_preset_slot(&mut self, slot: u8) -> Result<Preset> {
        ensure!(
            slot < PRESET_SLOTS,
            "preset slot {slot} out of range 0-{}",
            PRESET_SLOTS - 1
        );
        let address = Register::M0_V.address() + u16::from(slot) * 4;
        let words = self.read_words(address, 4).await?;
        let words: [Word; 4] = words
            .as_slice()
            .try_into()
            .context("short read for preset block")?;

        Ok(Preset::from_words(words, self.info.model))
    }

    /* Setters */

    /// Set the output voltage setpoint in volts.
    ///
    /// Scales using the model from the last [`Self::fetch_info`]; the cached
    /// [`Info::v_set`] is updated on success.
    ///
    /// # Errors
    /// Will return `Err` if the value is out of range or the write fails.
    pub async fn set_v_set(&mut self, volts: f32) -> Result<()> {
        let word = scaled_word(volts, self.info.model.v_mul())?;
        self.write_one(Register::V_SET, word).await?;
        self.info.v_set = volts;

        Ok(())
    }

    /// Set the output current setpoint in amps.
    ///
    /// Scales using the model (and current range on the RD6012P) from the last
    /// [`Self::fetch_info`]; the cached [`Info::i_set`] is updated on success.
    ///
    /// # Errors
    /// Will return `Err` if the value is out of range or the write fails.
    pub async fn set_i_set(&mut self, amps: f32) -> Result<()> {
        let word = scaled_word(amps, self.info.model.i_mul(self.info.i_range))?;
        self.write_one(Register::I_SET, word).await?;
        self.info.i_set = amps;

        Ok(())
    }

    /// Enable or disable the output.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_output(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OUTPUT, bool_word(on)).await?;
        self.info.output = on;

        Ok(())
    }

    /// Recall a preset slot, `0..=9`.
    ///
    /// # Errors
    /// Will return `Err` if the slot is out of range or the write fails.
    pub async fn set_preset(&mut self, slot: Word) -> Result<()> {
        ensure!(
            slot < u16::from(PRESET_SLOTS),
            "preset slot {slot} out of range 0-{}",
            PRESET_SLOTS - 1
        );
        self.write_one(Register::PRESET, slot).await?;
        self.info.preset = slot;

        Ok(())
    }

    /// Set the real-time clock.
    ///
    /// # Errors
    /// Will return `Err` if the model has no RTC or the write fails.
    pub async fn set_date_time(&mut self, dt: &DateTime) -> Result<()> {
        ensure!(
            self.info.model.has_rtc(),
            "{} has no real-time clock",
            self.info.model
        );
        self.write_words(Register::YEAR.address(), &dt.to_words())
            .await
    }

    /// Option: accept setpoints while the output is on.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_take_ok(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OPT_TAKE_OK, bool_word(on)).await
    }

    /// Option: recall setpoints when the output turns off.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_take_out(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OPT_TAKE_OUT, bool_word(on)).await
    }

    /// Option: restore the output state on power-on.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_boot_pow(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OPT_BOOT_POW, bool_word(on)).await
    }

    /// Option: buzzer.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_buzz(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OPT_BUZZ, bool_word(on)).await
    }

    /// Option: boot logo.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_logo(&mut self, on: bool) -> Result<()> {
        self.write_one(Register::OPT_LOGO, bool_word(on)).await
    }

    /// Option: display language index.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_lang(&mut self, lang: Word) -> Result<()> {
        self.write_one(Register::OPT_LANG, lang).await
    }

    /// Option: backlight level.
    ///
    /// # Errors
    /// Will return `Err` if the write fails.
    pub async fn set_light(&mut self, level: Word) -> Result<()> {
        self.write_one(Register::OPT_LIGHT, level).await
    }

    /* Device control */

    /// Reboot the unit into bootloader (flash) mode.
    ///
    /// The device usually resets without answering, so I/O/timeout errors are
    /// treated as expected: verify the result with
    /// [`bootloader::Bootloader::detect`], or let
    /// [`bootloader::Bootloader::enter`] handle the whole handshake.
    ///
    /// # Errors
    /// Will return `Err` only for serial open/flush failures.
    pub async fn reboot_to_bootloader(&mut self) -> Result<()> {
        let write = timeout(
            Duration::from_millis(500),
            self.ctx
                .write_single_register(Register::SYS_CMD.address(), CMD_REBOOT),
        );
        match write.await {
            Ok(result) => {
                // An answered error is unusual but not fatal; the reboot may
                // still have happened, but surface it so callers can decide.
                if let Err(err) = result {
                    bail!("device answered the reboot command with an error: {err}");
                }
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }

    /* Low-level wrappers */

    /// Read `quantity` holding registers starting at `address`.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn read_words(&mut self, address: Address, quantity: Quantity) -> Result<Words> {
        Ok(self.ctx.read_holding_registers(address, quantity).await?)
    }

    /// Read one holding register.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails.
    pub async fn read_one(&mut self, register: Register) -> Result<Word> {
        let mut words = self.read_words(register.address(), 1).await?;
        words
            .pop()
            .context("empty read response")
            .with_context(|| format!("reading {}", register.name()))
    }

    /// Read two consecutive holding registers, `[high, low]`.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus read fails or returns fewer than two
    /// registers.
    pub async fn read_pair(&mut self, register: Register) -> Result<WordPair> {
        let words = self.read_words(register.address(), 2).await?;
        words
            .as_slice()
            .try_into()
            .context("short read response")
            .with_context(|| format!("reading {} + 1", register.name()))
    }

    /// Write one holding register.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus write fails.
    pub async fn write_one(&mut self, register: Register, word: Word) -> Result<()> {
        self.ctx
            .write_single_register(register.address(), word)
            .await?;

        Ok(())
    }

    /// Write consecutive holding registers starting at `address`.
    ///
    /// # Errors
    /// Will return `Err` if the Modbus write fails.
    pub async fn write_words(&mut self, address: Address, words: &[Word]) -> Result<()> {
        self.ctx.write_multiple_registers(address, words).await?;

        Ok(())
    }
}

/// Convert a scaled physical value back into a raw register word.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scaled_word(value: f32, mul: f32) -> Result<Word> {
    ensure!(
        value.is_finite() && value >= 0.0,
        "value must be a non-negative finite number, got {value}"
    );
    let raw = value * mul;
    ensure!(
        raw <= f32::from(Word::MAX),
        "value {value} does not fit in a 16-bit register at scale {mul}"
    );

    Ok(raw.round() as Word)
}

const fn bool_word(on: bool) -> Word {
    if on {
        BOOL_TRUE
    } else {
        BOOL_FALSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_word_rounds_and_validates() {
        assert_eq!(scaled_word(4.20, 100.0).unwrap(), 420);
        assert_eq!(scaled_word(0.69, 1_000.0).unwrap(), 690);
        assert_eq!(scaled_word(0.456_7, 1_000.0).unwrap(), 457);
        assert!(scaled_word(-1.0, 100.0).is_err());
        assert!(scaled_word(f32::NAN, 100.0).is_err());
        assert!(scaled_word(700.0, 100.0).is_err());
    }

    #[test]
    fn bool_words() {
        assert_eq!(bool_word(true), 1);
        assert_eq!(bool_word(false), 0);
    }
}
