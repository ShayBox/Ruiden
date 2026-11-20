//! The stock bootloader / firmware update protocol.
//!
//! Ruiden units can reboot into a small bootloader that speaks a plain-ASCII
//! protocol over the same serial line.
//!
//! ```text
//! host -> "queryd\r\n"   bootloader -> "boot"        (only answers in flash mode)
//! host -> "getinf\r\n"   bootloader -> 13-byte "inf" + model/serial/firmware
//! host -> "upfirm\r\n"   bootloader -> "upredy"      (enter flash mode)
//! host -> <64-byte chunk> bootloader -> "OK"         (repeated until done)
//! ```
//!
//! Entering flash mode from the application is done over Modbus by writing
//! [`crate::register::CMD_REBOOT`] to [`crate::Register::SYS_CMD`]; the unit
//! usually answers with a single `0xFC` byte before resetting, or nothing at all.

use std::{path::Path, time::Duration};

use anyhow::{bail, ensure, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::{sleep, timeout, Instant},
};
use tokio_serial::SerialStream;

use crate::{
    info::serial_number,
    model::Model,
    register::{Register, CMD_REBOOT},
    Word,
};

/// Size of a firmware chunk accepted by the bootloader.
pub const BLOCK_SIZE: usize = 64;

const CMD_DETECT: &[u8] = b"queryd\r\n";
const CMD_INFO: &[u8] = b"getinf\r\n";
const CMD_FLASH: &[u8] = b"upfirm\r\n";
const RSP_BOOT: &[u8] = b"boot";
const RSP_READY: &[u8] = b"upredy";
const RSP_OK: &[u8] = b"OK";
const RSP_INFO: &[u8] = b"inf";

/// Wait this long for the unit to come back up after a reboot into flash mode.
pub const REBOOT_DELAY: Duration = Duration::from_secs(3);
/// Wait this long for a bootloader reply before assuming failure.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// CRC-16/MODBUS, as used by the Modbus RTU frames this module sends raw.
#[must_use]
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Device identification reported by the bootloader (`getinf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootInfo {
    /// Raw device ID (register 0 equivalent), e.g. `60_181`
    pub id: Word,
    /// Zero-padded serial number
    pub sn: String,
    /// Firmware version, `136` = v1.36
    pub fw: Word,
}

impl BootInfo {
    /// The device model decoded from [`Self::id`].
    #[must_use]
    pub const fn model(&self) -> Model {
        Model::from_id(self.id)
    }
}

/// A serial connection to a unit that is (or will be) in bootloader mode.
#[derive(Debug)]
pub struct Bootloader {
    stream: SerialStream,
}

impl Bootloader {
    /// Opens the serial port.
    ///
    /// # Errors
    /// Will return `Err` if the serial port cannot be opened.
    pub fn new(port: &str, baud_rate: u32) -> Result<Self> {
        let builder = tokio_serial::new(port, baud_rate);
        Ok(Self {
            stream: SerialStream::open(&builder)?,
        })
    }

    /// Writes a command and reads `len` bytes back, bounded by [`REPLY_TIMEOUT`].
    async fn command(&mut self, cmd: &[u8], len: usize) -> Result<Vec<u8>> {
        self.stream.write_all(cmd).await?;
        self.stream.flush().await?;
        let mut buf = vec![0u8; len];
        match timeout(REPLY_TIMEOUT, self.stream.read_exact(&mut buf)).await {
            Ok(Ok(_)) => Ok(buf),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Checks whether the unit is currently in bootloader (flash) mode.
    ///
    /// # Errors
    /// Will return `Err` on serial I/O failure.
    pub async fn detect(&mut self) -> Result<bool> {
        Ok(self.command(CMD_DETECT, RSP_BOOT.len()).await? == RSP_BOOT)
    }

    /// Reads the device identification from the bootloader.
    ///
    /// # Errors
    /// Will return `Err` if the unit does not answer with a valid `inf` block.
    pub async fn info(&mut self) -> Result<BootInfo> {
        let res = self.command(CMD_INFO, 13).await?;
        ensure!(res.len() == 13, "no response from bootloader");
        ensure!(
            &res[0..3] == RSP_INFO,
            "invalid bootloader response: {res:02x?}"
        );
        let sn = u32::from_le_bytes([res[3], res[4], res[5], res[6]]);
        let id = Word::from(res[7]) | (Word::from(res[8]) << 8);
        Ok(BootInfo {
            id,
            sn: serial_number(sn),
            fw: Word::from(res[11]),
        })
    }

    /// Reboots an application-mode unit into the bootloader over raw Modbus RTU.
    ///
    /// The unit typically answers with a stray `0xFC` byte (a truncated Modbus
    /// reply) and resets without a proper response, so a missing/odd reply is
    /// not treated as an error — use [`Self::detect`] afterwards to verify.
    ///
    /// # Errors
    /// Will return `Err` on serial I/O failure.
    pub async fn reboot(&mut self, slave_id: u8) -> Result<()> {
        let mut frame = vec![slave_id, 0x06];
        frame.extend_from_slice(&Register::SYS_CMD.address().to_be_bytes());
        frame.extend_from_slice(&CMD_REBOOT.to_be_bytes());
        frame.extend_from_slice(&crc16(&frame).to_le_bytes());

        self.stream.write_all(&frame).await?;
        self.stream.flush().await?;
        // Drain whatever the dying application sends; ignore timeouts.
        let _ = timeout(Duration::from_secs(1), self.stream.read_u8()).await;
        Ok(())
    }

    /// Waits for the unit to appear in bootloader mode after a reboot.
    ///
    /// # Errors
    /// Will return `Err` if the bootloader does not answer before the deadline.
    pub async fn wait_for_bootloader(&mut self, deadline: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            if self.detect().await? {
                return Ok(());
            }
            ensure!(
                started.elapsed() < deadline,
                "unit did not enter bootloader mode"
            );
            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Starts a firmware transfer and uploads the whole image in
    /// [`BLOCK_SIZE`]-byte chunks.
    ///
    /// `progress` is called with the number of bytes uploaded so far.
    ///
    /// # Errors
    /// Will return `Err` if the unit is not in flash mode, a chunk is not
    /// acknowledged, or I/O fails.
    pub async fn flash(&mut self, firmware: &[u8], mut progress: impl FnMut(usize)) -> Result<()> {
        let res = self.command(CMD_FLASH, RSP_READY.len()).await?;
        ensure!(res == RSP_READY, "failed to enter flash mode: {res:02x?}");

        for (pos, chunk) in firmware.chunks(BLOCK_SIZE).enumerate() {
            self.stream.write_all(chunk).await?;
            self.stream.flush().await?;
            let mut ack = [0u8; 2];
            match timeout(REPLY_TIMEOUT, self.stream.read_exact(&mut ack)).await {
                Ok(Ok(_)) => ensure!(
                    ack == *RSP_OK,
                    "flash failed at chunk {pos}: bad ack {ack:02x?}"
                ),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => bail!("no acknowledgement for chunk {pos}"),
            }
            progress(pos * BLOCK_SIZE + chunk.len());
        }
        Ok(())
    }

    /// Opens the port, reboots the unit into flash mode when needed, verifies
    /// the bootloader and returns it, ready for [`Self::flash`].
    ///
    /// # Errors
    /// Will return `Err` if the unit cannot be reached or does not enter
    /// bootloader mode.
    pub async fn enter(port: &str, baud_rate: u32, slave_id: u8) -> Result<Self> {
        let mut boot = Self::new(port, baud_rate)?;
        if !boot.detect().await? {
            boot.reboot(slave_id).await?;
            sleep(REBOOT_DELAY).await;
            boot.wait_for_bootloader(REPLY_TIMEOUT).await?;
        }
        Ok(boot)
    }

    /// Full firmware update: enters flash mode, reports the bootloader's view
    /// of the device, uploads `firmware` and prints progress via `progress`.
    ///
    /// # Errors
    /// Will return `Err` if any step of the update fails.
    pub async fn update_firmware(
        port: &str,
        baud_rate: u32,
        slave_id: u8,
        firmware: &Path,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<BootInfo> {
        let firmware = std::fs::read(firmware)?;
        let mut boot = Self::enter(port, baud_rate, slave_id).await?;
        let info = boot.info().await?;
        boot.flash(&firmware, |done| progress(done, firmware.len()))
            .await?;
        Ok(info)
    }
}
