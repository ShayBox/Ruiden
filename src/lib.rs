use anyhow::{Error, Result};
use register::Register;
use serialize::{HighLowPair, Information, SerialNumber};
use tokio_modbus::{client::Context, prelude::*, Address, Quantity};
use tokio_serial::SerialStream;

pub mod register;
pub mod serialize;

pub type Word = u16;
pub type Words = Vec<Word>;
pub type WordPair = [Word; 2];

pub struct Ruiden {
    pub ctx:  Context,
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
            info: Information::default(),
        })
    }

    /* Fetchers */

    /// # Errors
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_info(&mut self) -> Result<&Information> {
        let address = Register::ID as Address;
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
    /// Will return `Err` if `Self::read_pair` errors
    pub async fn get_sn(&mut self) -> Result<String> {
        let high_low_pair = self.read_pair(Register::SN_H).await?;
        let serial_number = SerialNumber::from(high_low_pair);

        Ok(serial_number.0)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_fw(&mut self) -> Result<u16> {
        self.read_one(Register::FW).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_pair` errors
    pub async fn get_int_c(&mut self) -> Result<u16> {
        self.read_pair(Register::INT_C_S).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_pair` errors
    pub async fn get_int_f(&mut self) -> Result<u16> {
        self.read_pair(Register::INT_F_S).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_v_set(&mut self) -> Result<f32> {
        let v_set = self.read_one(Register::V_SET).await?;
        Ok(f32::from(v_set) / self.info.v_mul)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_i_set(&mut self) -> Result<f32> {
        let i_set = self.read_one(Register::I_SET).await?;
        Ok(f32::from(i_set) / self.info.i_mul)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_v_out(&mut self) -> Result<f32> {
        let v_out = self.read_one(Register::V_OUT).await?;
        Ok(f32::from(v_out) / self.info.v_mul)
    }

    /// # Errors
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_i_out(&mut self) -> Result<f32> {
        let i_out = self.read_one(Register::I_OUT).await?;
        Ok(f32::from(i_out) / self.info.i_mul)
    }

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
    /// Will return `Err` if `Self::read_pair` errors
    pub async fn get_ext_c(&mut self) -> Result<u16> {
        self.read_pair(Register::EXT_C_S).await
    }

    /// # Errors
    /// Will return `Err` if `Self::read_pair` errors
    pub async fn get_ext_f(&mut self) -> Result<u16> {
        self.read_pair(Register::EXT_F_S).await
    }

    // TODO: AH
    // TODO: WH

    /* Setters */

    /* Updaters */

    /* Wrappers */

    /// # Errors
    /// Will return `Err` if `Context::read_multiple` errors
    pub async fn read_pair(&mut self, register: Register) -> Result<u16> {
        let words = self.read_multiple(register as Address, 2).await?;
        let word_pair = WordPair::try_from(&words[0..=1])?;
        let high_low_pair = HighLowPair::from(word_pair);

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
}
