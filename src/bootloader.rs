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
//! The reboot is sent at the application's baud, but the bootloader itself
//! always comes back at the fixed [`BOOTLOADER_BAUD`] (115200), independent of
//! that — so [`Bootloader::enter`] reopens the port at 115200 for everything
//! after the reboot.

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

/// Baud rate the stock flash bootloader always speaks at.
///
/// Unlike the application — whose Modbus baud is user-configurable, up to
/// `115_200` on the USB interface — the bootloader is hard-wired to `115_200`. A
/// higher application baud still reboots the unit, but every bootloader command
/// after that (and an already-flashing unit) must use this rate, or the unit
/// looks dead. (Ref: the `EEVblog` RD6006 thread — "bootloader uses fixed
/// baudrate 115200".)
pub const BOOTLOADER_BAUD: u32 = 115_200;

/// Wait this long after sending the reboot command before polling for the
/// bootloader (the unit needs a moment to reset and bring its UART back up).
pub const REBOOT_DELAY: Duration = Duration::from_secs(3);
/// Overall time to spend waiting for the unit to answer in bootloader mode.
pub const BOOTLOADER_TIMEOUT: Duration = Duration::from_secs(15);
/// Wait this long for a bootloader reply before assuming failure.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// Per-attempt read timeout while polling for the bootloader. Kept short so
/// [`Bootloader::wait_for_bootloader`] can re-send its probe several times
/// while the unit finishes resetting, instead of one attempt eating the whole
/// deadline.
const DETECT_TIMEOUT: Duration = Duration::from_millis(1_000);
/// Pause between bootloader detection attempts.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

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
        self.try_command(cmd, len, REPLY_TIMEOUT).await
    }

    /// [`Self::command`] with an explicit reply timeout.
    async fn try_command(&mut self, cmd: &[u8], len: usize, wait: Duration) -> Result<Vec<u8>> {
        self.stream.write_all(cmd).await?;
        self.stream.flush().await?;
        let mut buf = vec![0u8; len];
        match timeout(wait, self.stream.read_exact(&mut buf)).await {
            Ok(Ok(_)) => Ok(buf),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Discards whatever is already sitting in the receive buffer — a framing
    /// glitch emitted while the MCU reset, the dying application's truncated
    /// Modbus reply, or a previous attempt's partial response — so those bytes
    /// cannot shift the next framed read off the start of a reply.
    async fn drain_input(&mut self) {
        let mut buf = [0u8; 256];
        loop {
            match timeout(Duration::from_millis(50), self.stream.read(&mut buf)).await {
                Ok(Ok(0) | Err(_)) | Err(_) => break,
                Ok(Ok(_)) => {}
            }
        }
    }

    /// Checks whether the unit is currently in bootloader (flash) mode.
    ///
    /// # Errors
    /// Will return `Err` on serial I/O failure.
    pub async fn detect(&mut self) -> Result<bool> {
        self.drain_input().await;
        let res = self
            .try_command(CMD_DETECT, RSP_BOOT.len(), DETECT_TIMEOUT)
            .await?;
        Ok(res == RSP_BOOT)
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
        // Drain whatever the dying application sends before it resets (usually a
        // stray `0xFC`); ignore the timeout, the unit may answer nothing at all.
        self.drain_input().await;
        Ok(())
    }

    /// Waits for the unit to appear in bootloader mode after a reboot.
    ///
    /// Polls [`Self::detect`] until the unit answers or `deadline` elapses, so a
    /// probe sent while the unit is still resetting is retried instead of
    /// failing the whole update.
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
            sleep(POLL_INTERVAL).await;
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
    /// `baud_rate` is the application's Modbus baud, used only to issue the
    /// reboot. The returned connection always speaks [`BOOTLOADER_BAUD`], so a
    /// non-default application baud still flashes correctly.
    ///
    /// # Errors
    /// Will return `Err` if the unit cannot be reached or does not enter
    /// bootloader mode.
    pub async fn enter(port: &str, baud_rate: u32, slave_id: u8) -> Result<Self> {
        // The bootloader always answers at BOOTLOADER_BAUD, so check for an
        // already-flashing unit (e.g. one entered by holding ENTER at power-on)
        // at that rate, not at the application baud.
        let mut boot = Self::new(port, BOOTLOADER_BAUD)?;
        if boot.detect().await? {
            return Ok(boot);
        }

        // Not in flash mode: reboot it over Modbus at the application baud. When
        // that differs from the bootloader baud, hand the port to a short-lived
        // handle at the application baud, then take it back at BOOTLOADER_BAUD.
        if baud_rate == BOOTLOADER_BAUD {
            boot.reboot(slave_id).await?;
        } else {
            drop(boot);
            let mut app = Self::new(port, baud_rate)?;
            app.reboot(slave_id).await?;
            drop(app);
            boot = Self::new(port, BOOTLOADER_BAUD)?;
        }

        sleep(REBOOT_DELAY).await;
        boot.wait_for_bootloader(BOOTLOADER_TIMEOUT).await?;
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
