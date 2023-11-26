#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use anyhow::{Error, Result};
use derivative::Derivative;
use register::Register;
use serialize::{Information, Initialization};
use tokio_modbus::{client::Context, prelude::*, Address, Quantity};
use tokio_serial::SerialStream;

pub mod register;
pub mod serialize;

pub type Word = u16;
pub type Words = Vec<Word>;
pub type WordPair = (Word, Word);

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
    ///
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
    ///
    /// Will return `Err` if `Context::read_holding_registers` errors
    pub async fn read_multiple(&mut self, address: Address, quantity: Quantity) -> Result<Words> {
        self.ctx
            .read_holding_registers(address, quantity)
            .await
            .map_err(Error::msg)
    }

    /// # Errors
    ///
    /// Will return `Err` if `Self::read` errors
    pub async fn read_one(&mut self, register: Register) -> Result<Word> {
        Ok(self.read_multiple(register as Address, 1).await?[0])
    }

    /// # Errors
    ///
    /// Will return `Err` if `Context::write_multiple_registers` errors
    pub async fn write_multiple(&mut self, register: Register, words: Words) -> Result<()> {
        self.ctx
            .write_multiple_registers(register as Address, &words)
            .await
            .map_err(Error::msg)
    }

    /// # Errors
    ///
    /// Will return `Err` if `Context::write_single_register` errors
    pub async fn write_one(&mut self, register: Register, word: Word) -> Result<()> {
        self.ctx
            .write_single_register(register as Address, word)
            .await
            .map_err(Error::msg)
    }

    /* Fetchers */

    /// # Errors
    ///
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_all(&mut self) -> Result<()> {
        let address = Register::ID as Address;
        let quantity = Register::I_RANGE as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.init = Into::<Initialization>::into(words[0..=3].to_vec());
        self.info = Into::<Information>::into(words[4..].to_vec());

        Ok(())
    }

    /// # Errors
    ///
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_init(&mut self) -> Result<&Initialization> {
        let address = Register::ID as Address;
        let quantity = Register::FW as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.init = Into::<Initialization>::into(words);

        Ok(&self.init)
    }

    /// # Errors
    ///
    /// Will return `Err` if `Self::read_multiple` errors
    pub async fn fetch_info(&mut self) -> Result<&Information> {
        let address = Register::INT_C_S as Address;
        let quantity = Register::I_RANGE as Address - address + 1;
        let words = self.read_multiple(address, quantity).await?;
        self.info = Into::<Information>::into(words);

        Ok(&self.info)
    }

    /* Getters */

    /// # Errors
    ///
    /// Will return `Err` if `Self::read_one` errors
    pub async fn get_id(&mut self) -> Result<u16> {
        self.read_one(Register::ID).await
    }

    /* Setters */

    /* Updaters */
}
