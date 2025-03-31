use arbitrary_int::u2;

/// Transmitter Holding Register.
#[bitbybit::bitfield(u32)]
pub struct Fifo {
    #[bits(0..=7, rw)]
    data: u8,
}

#[bitbybit::bitfield(u32)]
pub struct Ier {
    /// Enable Modem Status Interrupt
    #[bit(3, rw)]
    modem_status: bool,
    /// Enable Receiver Line Status Interrupt
    #[bit(2, rw)]
    line_status: bool,
    /// Enable Transmitter Holding Register Empty Interrupt
    #[bit(1, rw)]
    thr_empty: bool,
    /// Enable Received Data Available Interrupt
    #[bit(0, rw)]
    rx_avl: bool,
}

/// Interrupt identification ID
#[bitbybit::bitenum(u3, exhaustive = false)]
#[derive(Debug, PartialEq, Eq)]
pub enum IntId2 {
    ReceiverLineStatus = 0b011,
    RxDataAvailable = 0b010,
    CharTimeout = 0b110,
    ThrEmpty = 0b001,
    ModemStatus = 0b000,
}

/// Interrupt Identification Register
#[bitbybit::bitfield(u32)]
pub struct Iir {
    /// 16550 mode enabled?
    #[bits(6..=7, r)]
    fifo_enabled: u2,
    #[bits(1..=3, r)]
    int_id: Option<IntId2>,
    /// Interrupt Pending, active low.
    #[bit(0, r)]
    int_pend_n: bool,
}

#[bitbybit::bitenum(u2, exhaustive = true)]
pub enum RxFifoTrigger {
    OneByte = 0b00,
    FourBytes = 0b01,
    EightBytes = 0b10,
    FourteenBytes = 0b11,
}

impl RxFifoTrigger {
    pub const fn as_num(self) -> u32 {
        match self {
            RxFifoTrigger::OneByte => 1,
            RxFifoTrigger::FourBytes => 4,
            RxFifoTrigger::EightBytes => 8,
            RxFifoTrigger::FourteenBytes => 14,
        }
    }
}

/// FIFO Control Register
#[bitbybit::bitfield(u32, default = 0x0)]
pub struct Fcr {
    #[bits(4..=5, rw)]
    rx_fifo_trigger: RxFifoTrigger,
    #[bit(3, rw)]
    dma_mode_sel: bool,
    #[bit(2, rw)]
    reset_tx_fifo: bool,
    #[bit(1, rw)]
    reset_rx_fifo: bool,
    #[bit(0, rw)]
    fifo_enable: bool,
}

#[bitbybit::bitenum(u2, exhaustive = true)]
#[derive(Default, Debug, PartialEq, Eq)]
pub enum WordLen {
    Five = 0b00,
    Six = 0b01,
    Seven = 0b10,
    #[default]
    Eight = 0b11,
}

#[bitbybit::bitenum(u1, exhaustive = true)]
#[derive(Default, Debug, PartialEq, Eq)]
pub enum StopBits {
    #[default]
    One = 0b0,
    /// 1.5 for 5 bits/char, 2 otherwise.
    OnePointFiveOrTwo = 0b1,
}

/// Line control register
#[bitbybit::bitfield(u32, default = 0x00)]
pub struct Lcr {
    #[bit(7, rw)]
    div_access_latch: bool,
    #[bit(6, rw)]
    set_break: bool,
    #[bit(5, rw)]
    stick_parity: bool,
    #[bit(4, rw)]
    even_parity: bool,
    #[bit(3, rw)]
    parity_enable: bool,
    /// 0: 1 stop bit, 1: 2 stop bits or 1.5 if 5 bits/char selected
    #[bit(2, rw)]
    stop_bits: StopBits,
    #[bits(0..=1, rw)]
    word_len: WordLen,
}

impl Lcr {
    pub fn new_for_divisor_access() -> Self {
        Self::new_with_raw_value(0x80)
    }
}

/// Line Status Register
#[bitbybit::bitfield(u32)]
#[derive(Debug)]
pub struct Lsr {
    #[bit(7, rw)]
    error_in_rx_fifo: bool,
    /// In the FIFO mode, this is set to 1 when the TX FIFO and shift register are both empty.
    #[bit(6, rw)]
    tx_empty: bool,
    /// In the FIFO mode, this is set to 1 when the TX FIFO is empty. There might still be a byte
    /// in the TX shift register.
    #[bit(5, rw)]
    thr_empty: bool,
    #[bit(4, rw)]
    break_interrupt: bool,
    #[bit(3, rw)]
    framing_error: bool,
    #[bit(2, rw)]
    parity_error: bool,
    #[bit(1, rw)]
    overrun_error: bool,
    #[bit(0, rw)]
    data_ready: bool,
}

#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct AxiUart16550 {
    _reserved: [u32; 0x400],
    /// FIFO register for LCR[7] == 0 or Divisor Latch (LSB) register for LCR[7] == 1
    fifo_or_dll: u32,
    /// Interrupt Enable Register for LCR[7] == 0 or Divisor Latch (MSB) register for LCR[7] == 1
    ier_or_dlm: u32,
    /// Interrupt Identification Register or FIFO Control Register. FCR is not included in 16450
    /// mode. If LCR[7] == 1, this register will be the read-only FIFO control register.
    /// If LCR[7] == 0, this register will be the read-only interrupt IIR register or the
    /// write-only FIFO control register.
    iir_or_fcr: u32,
    /// Line Control Register
    lcr: Lcr,
    /// Modem Control Register
    mcr: u32,
    /// Line Status Register
    lsr: Lsr,
    /// Modem Status Register
    msr: u32,
    /// Scratch Register
    scr: u32,
}
