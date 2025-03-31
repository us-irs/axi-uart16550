use core::convert::Infallible;

use crate::{
    DEFAULT_RX_TRIGGER_LEVEL,
    registers::{self, Fcr, Ier, Iir, IntId2, Lsr},
};

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct RxErrors {
    parity: bool,
    frame: bool,
    overrun: bool,
}

impl RxErrors {
    pub const fn new() -> Self {
        Self {
            parity: false,
            frame: false,
            overrun: false,
        }
    }

    pub const fn parity(&self) -> bool {
        self.parity
    }

    pub const fn frame(&self) -> bool {
        self.frame
    }

    pub const fn overrun(&self) -> bool {
        self.overrun
    }

    pub const fn has_errors(&self) -> bool {
        self.parity || self.frame || self.overrun
    }
}

pub struct Rx {
    /// Internal MMIO register structure.
    pub(crate) regs: registers::MmioAxiUart16550<'static>,
    pub(crate) errors: Option<RxErrors>,
}

impl Rx {
    /// Steal the RX part of the UART 16550.
    ///
    /// You should only use this if you can not use the regular [super::AxiUart16550] constructor
    /// and the [super::AxiUart16550::split] method.
    ///
    /// This function assumes that the setup of the UART was already done.
    /// It can be used to create an RX handle inside an interrupt handler without having to use
    /// a [critical_section::Mutex] if the user can guarantee that the RX handle will only be
    /// used by the interrupt handler or only interrupt specific API will be used.
    ///
    /// # Safety
    ///
    /// The same safey rules specified in [super::AxiUart16550::new] apply.
    pub const unsafe fn steal(base_addr: usize) -> Self {
        Self {
            regs: unsafe { registers::AxiUart16550::new_mmio_at(base_addr) },
            errors: None,
        }
    }

    pub(crate) fn new(regs: registers::MmioAxiUart16550<'static>) -> Self {
        Self { regs, errors: None }
    }

    #[inline]
    pub fn read_fifo(&mut self) -> nb::Result<u8, Infallible> {
        let status_reg = self.regs.read_lsr();
        if !status_reg.data_ready() {
            return Err(nb::Error::WouldBlock);
        }
        if status_reg.error_in_rx_fifo() {
            self.errors = Some(Self::lsr_to_errors(status_reg));
        }
        Ok(self.read_fifo_unchecked())
    }

    #[inline(always)]
    pub fn read_fifo_unchecked(&mut self) -> u8 {
        self.regs.read_fifo_or_dll() as u8
    }

    /// Start interrupt driven reception.
    ///
    /// This function resets the FIFO with [Self::reset_fifo] and then enables the interrupts
    /// with [Self::enable_interrupt].
    /// After this, you only need to call [Self::on_interrupt_receiver_line_status] and
    /// [Self::on_interrupt_data_available_or_char_timeout] in your interrupt handler depending
    /// on the value of the IIR register to continously receive data.
    #[inline]
    pub fn start_interrupt_driven_reception(&mut self) {
        self.reset_fifo();
        self.enable_interrupt();
    }

    #[inline]
    pub fn enable_interrupt(&mut self) {
        self.regs.modify_ier_or_dlm(|val| {
            let mut ier = Ier::new_with_raw_value(val);
            ier.set_rx_avl(true);
            ier.set_line_status(true);
            ier.raw_value()
        });
    }

    #[inline]
    pub fn disable_interrupt(&mut self) {
        self.regs.modify_ier_or_dlm(|val| {
            let mut ier = Ier::new_with_raw_value(val);
            ier.set_rx_avl(false);
            ier.set_line_status(false);
            ier.raw_value()
        });
    }

    #[inline]
    pub fn reset_fifo(&mut self) {
        self.regs.write_iir_or_fcr(
            Fcr::builder()
                .with_rx_fifo_trigger(DEFAULT_RX_TRIGGER_LEVEL)
                .with_dma_mode_sel(false)
                .with_reset_tx_fifo(false)
                .with_reset_rx_fifo(true)
                .with_fifo_enable(true)
                .build()
                .raw_value(),
        );
    }

    #[inline(always)]
    pub fn has_data(&mut self) -> bool {
        self.regs.read_lsr().data_ready()
    }

    #[inline]
    pub fn read_iir(&mut self) -> Iir {
        Iir::new_with_raw_value(self.regs.read_iir_or_fcr())
    }

    #[inline]
    pub fn on_interrupt_receiver_line_status(&mut self, _iir: Iir) -> RxErrors {
        let lsr = self.regs.read_lsr();
        Self::lsr_to_errors(lsr)
    }

    #[inline]
    pub fn on_interrupt_data_available_or_char_timeout(
        &mut self,
        int_id2: IntId2,
        buf: &mut [u8; 16],
    ) -> usize {
        let mut read = 0;
        // It is guaranteed that we can read the FIFO trigger level.
        if int_id2 == IntId2::RxDataAvailable {
            let trigger_level = Fcr::new_with_raw_value(self.regs.read_iir_or_fcr());
            (0..trigger_level.rx_fifo_trigger().as_num() as usize).for_each(|i| {
                buf[i] = self.read_fifo_unchecked();
                read += 1;
            });
        }
        // Read the rest of the FIFO.
        while self.has_data() && read < 16 {
            buf[read] = self.read_fifo_unchecked();
            read += 1;
        }
        read
    }

    pub fn lsr_to_errors(status_reg: Lsr) -> RxErrors {
        let mut errors = RxErrors::new();
        if status_reg.framing_error() {
            errors.frame = true;
        }
        if status_reg.parity_error() {
            errors.parity = true;
        }
        if status_reg.overrun_error() {
            errors.overrun = true;
        }
        errors
    }
}

impl embedded_hal_nb::serial::ErrorType for Rx {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Read for Rx {
    #[inline]
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        self.read_fifo()
    }
}

impl embedded_io::ErrorType for Rx {
    type Error = Infallible;
}

impl embedded_io::Read for Rx {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        while !self.has_data() {}
        let mut read = 0;
        for byte in buf.iter_mut() {
            match self.read_fifo() {
                Ok(data) => {
                    *byte = data;
                    read += 1;
                }
                Err(nb::Error::WouldBlock) => break,
            }
        }
        Ok(read)
    }
}
