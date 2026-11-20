# Ruiden

A Rust library and command-line tool for [Ruiden RD-series bench power
supplies](https://riden.ruidenbdc.com/), speaking Modbus RTU over a serial (USB/RS-485) link.

## Features

- **Model auto-detection** — device ID and per-model scale factors are read
  from the unit; physical values come back already typed as volts, amps and
  watts.
- **Async Modbus client** built on [`tokio-modbus`](https://crates.io/crates/tokio-modbus),
  with bulk block reads so polling the full state costs two transactions, not dozens.
- **Correct decoding** — signed temperatures (sign-flag registers), 32-bit
  Ah/Wh counters, enum-typed status (CV/CC, OVP/OCP, presets).
- **Full control API** — setpoints, output enable, preset recall and slot
  reads, real-time clock, device options.
- **Firmware updating** — the stock bootloader protocol (detect, reboot,
  identify, chunked upload) is implemented directly, no vendor tools required.

## Supported devices

| Model    | Device ID   | Voltage | Current         | Power | Notes                      |
|----------|-------------|---------|-----------------|-------|----------------------------|
| RD6006   | 60060–60064 | ÷100    | ÷1000           | ÷100  |                            |
| RD6006P  | 60065       | ÷1000   | ÷10000          | ÷1000 |                            |
| RK6006   | 60066       | ÷100    | ÷1000           | ÷100  | battery variant, no RTC    |
| RD6012   | 60120–60124 | ÷100    | ÷100            | ÷100  |                            |
| RD6012P  | 60125–60129 | ÷1000   | ÷10000 / ÷1000¹ | ÷1000 | ¹ depends on current range |
| RD6018/W | 60180–60189 | ÷100    | ÷100            | ÷100  |                            |
| RD6024   | 60240–60249 | ÷100    | ÷100            | ÷100  |                            |
| RD6030   | 60300–60309 | ÷100    | ÷100            | ÷100  | scale factors unverified   |

## Installation

As a library:

```toml
[dependencies]
ruiden = { git = "https://github.com/ShayBox/Ruiden" }
```

As a CLI:

```console
$ cargo install --git https://github.com/ShayBox/Ruiden
```

## Usage

### Library

```rust
use anyhow::Result;
use ruiden::Ruiden;

#[tokio::main]
async fn main() -> Result<()> {
    // Opens the port and identifies the unit.
    let mut psu = Ruiden::connect("/dev/ttyUSB0", 115_200, 1).await?;
    println!("Found {}", psu.info.model);

    // Refresh every measurement in two bulk reads.
    psu.update().await?;
    println!("{:.2} V / {:.3} A", psu.info.v_out, psu.info.i_out);

    // Setpoints are scaled using the detected model.
    psu.set_v_set(4.20).await?;
    psu.set_i_set(0.69).await?;
    psu.set_output(true).await?;
    Ok(())
}
```

Prefer the block fetchers (`fetch_info`, `fetch_status`, `update`) over the
single-register getters when polling; each getter is its own Modbus
transaction. Full API documentation lives in the crate docs (`cargo doc --open`).

### CLI

```console
$ ruiden info                          # identification + live measurements
$ ruiden set --v-set 4.2 --i-set 0.69 --output on
$ ruiden dump                          # all 256 registers, with names
$ ruiden flash RD60181_V1.36.bin       # reboot to bootloader, then update
```

Common options apply to every subcommand: `--port` (default `/dev/ttyUSB0`),
`--baud-rate` (default `115200`), `--slave-id` (default `1`).

## Notes & safety

- **Calibration registers** (55–62) and their NVRAM commit register (54) have
  no high-level API on purpose. A miscalibrated supply can be dangerous; the
  low-level `read_words`/`write_words` helpers exist if you really need them.
- **Firmware:** some stock firmware revisions break Modbus over USB. The
  community *UniSoft* build (linked from the ShayBox/Riden README) is known to
  work and can be flashed with `ruiden flash`. A failed update is usually
  recoverable: hold **ENTER** while powering on to reach the bootloader.
- **RD6012P** current resolution depends on the selected range; this is
  handled automatically once the device has been identified.
- Temperature registers encode the sign in the *preceding* register (`0` = positive, `1` = negative); decoding handles
  this transparently.