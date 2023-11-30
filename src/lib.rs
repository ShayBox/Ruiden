use anyhow::{Error, Result};
use derivative::Derivative;
use register::Register;
use serialize::{HighLowPair, Information, Initialization, SerialNumber};
use tokio_modbus::{client::Context, prelude::*, Address, Quantity};
use tokio_serial::SerialStream;

pub mod register;
pub mod serialize;

pub type Word = u16;
pub type Words = Vec<Word>;
pub type WordPair = [Word; 2];

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Ruiden {
    #[derivative(Debug = "ignore")]
    pub ctx:  Context,
    pub init: Initialization,
    pub info: Information,
}

impl Ruiden {
    /* Constructors */

    /// # Errors
    /// Will return `Err` if `SerialStream::open` errors
    pub fn new<'a>(
        path: impl Into<std::borrow::Cow<'a, str>>,
        baud_rate: u32,
        slave_id: SlaveId,
    ) -> Result<Self> {
        let builder = tokio_serial::new(path, baud_rate);
        let transport = SerialStream::open(&builder)?;
        let slave = Slave(slave_id);
        let ctx = rtu::attach_slave(transport, slave);

        Ok(Self {
            ctx,
            init: Initialization::default(),
            info: Information::default(),
        })
    }

    /* Wrappers */

    /// # Errors
    /// Will return `Err` if `Context::read_multiple` errors
    pub async fn read_high_low(&mut self, register: Register) -> Result<u16> {
        let words = self.read_multiple(register as Address, 2).await?;
        let word_pair = TryInto::<WordPair>::try_into(&words[0..=1])?;
        let high_low_pair = Into::<HighLowPair>::into(word_pair);

        Ok(high_low_pair.0)
    }

    /// # Errors
    /// Will return `Err` if `Context::read_holding_registers` errors
    pub async fn read_multiple(&mut self, address: Address, quantity: Quantity) -> Result<Words> {
        self.ctx
            .read_holding_registers(address, quantity)
            .await
            .map_err(Error::msg)
    }

    /// # Errors
    /// Will return `Err` if `Self::read` errors
    pub async fn read_one(&mut self, register: Register) -> Result<Word> {
        Ok(self.read_multiple(register as Address, 1).await?[0])
    }

    /// # Errors
    /// Will return `Err` if `Context::write_multiple_registers` errors
    pub async fn write_multiple(&mut self, register: Register, words: Words) -> Result<()> {
        self.ctx
            .write_multiple_registers(register as Address, &words)
            .await
            .map_err(Error::msg)
    }

    /// # Errors
    /// Will return `Err` if `Context::write_single_register` errors
    pub async fn write_one(&mut self, register: Register, word: Word) -> Result<()> {
        self.ctx
            .write_single_register(register as Address, word)
            .await
            .map_err(Error::msg)
    }

    /* Fetchers */

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_all(&mut self) -> Result<()> {
        let address = Register::ID as Address;
        let quantity = Register::I_RANGE as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.init = words[0..=3].to_vec().try_into()?;
        self.info = words[4..].to_vec().try_into()?;

        Ok(())
    }

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_init(&mut self) -> Result<&Initialization> {
        let address = Register::ID as Address;
        let quantity = Register::FW as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.init = words.try_into()?;

        Ok(&self.init)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_info(&mut self) -> Result<&Information> {
        let address = Register::INT_C_S as Address;
        let quantity = Register::I_RANGE as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.info = words.try_into()?;

        Ok(&self.info)
    }

    /* Getters */

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_id(&mut self) -> Result<u16> {
        self.read_one(Register::ID).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn get_sn(&mut self) -> Result<String> {
        let high_low_pair = self.read_high_low(Register::SN_H).await?;
        let serial_number = Into::<SerialNumber>::into(high_low_pair);

        Ok(serial_number.0)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_fw(&mut self) -> Result<u16> {
        self.read_one(Register::FW).await
    }

    /// # Errors
    /// Will return `Err` if `Self::get_high_low_pair` errors
    pub async fn get_int_c(&mut self) -> Result<u16> {
        self.read_high_low(Register::INT_C_S).await
    }

    /// # Errors
    /// Will return `Err` if `Self::get_high_low_pair` errors
    pub async fn get_int_f(&mut self) -> Result<u16> {
        self.read_high_low(Register::INT_F_S).await
    }

    // TODO: V_SET
    // TODO: I_SET
    // TODO: V_OUT
    // TODO: I_OUT
    // TODO: AH
    // TODO: P_OUT
    // TODO: V_IN
    // TODO: KEYPAD
    // TODO: OVP_OCP
    // TODO: OUTPUT
    // TODO: PRESET
    // TODO: I_RANGE
    // TODO: BAT_MODE
    // TODO: V_BAT

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn get_ext_c(&mut self) -> Result<u16> {
        self.read_high_low(Register::EXT_C_S).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn get_ext_f(&mut self) -> Result<u16> {
        self.read_high_low(Register::EXT_F_S).await
    }

    // TODO: AH
    // TODO: WH

    /* Setters */

    /* Updaters */
}
