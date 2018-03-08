//! Implementation of DMA-based SPI master and slave communication for the
//! SAM4L.
//!
//! Driver for the SPI hardware (separate from the USARTS), described in chapter
//! 26 of the datasheet.
//!
//! - Authors: Sam Crow <samcrow@uw.edu>, Philip Levis <pal@cs.stanford.edu>

use core::cell::Cell;
use core::cmp;

use dma::DMAChannel;
use dma::DMAClient;
use dma::DMAPeripheral;
use kernel::ReturnCode;

use kernel::common::VolatileCell;

use clock_pm;
use kernel::hil::clock_pm::{ClockManager,ClockParams,ClockClient};
use kernel::common::list::*;
use kernel::hil;
use kernel::hil::spi;
use kernel::hil::spi::ClockPhase;
use kernel::hil::spi::ClockPolarity;
use kernel::hil::spi::SpiMasterClient;
use kernel::hil::spi::SpiSlaveClient;
use pm;
use kernel::common::take_cell::TakeCell;

/// The registers used to interface with the hardware
#[repr(C, packed)]
struct SpiRegisters {
    cr: VolatileCell<u32>, //       0x0
    mr: VolatileCell<u32>, //       0x4
    rdr: VolatileCell<u32>, //      0x8
    tdr: VolatileCell<u32>, //      0xC
    sr: VolatileCell<u32>, //       0x10
    ier: VolatileCell<u32>, //      0x14
    idr: VolatileCell<u32>, //      0x18
    imr: VolatileCell<u32>, //      0x1C
    _reserved0: [u32; 4], //        0x20, 0x24, 0x28, 0x2C
    csr0: VolatileCell<u32>, //     0x30
    csr1: VolatileCell<u32>, //     0x34
    csr2: VolatileCell<u32>, //     0x38
    csr3: VolatileCell<u32>, //     0x3C
    _reserved1: [u32; 41], //       0x40 - 0xE0
    wpcr: VolatileCell<u32>, //     0xE4
    wpsr: VolatileCell<u32>, //     0xE8
    _reserved2: [u32; 3], //        0xEC - 0xF4
    features: VolatileCell<u32>, // 0xF8
    version: VolatileCell<u32>, //  0xFC
}

#[allow(unused_variables,dead_code)]
// Per-register masks defined in the SPI manual in chapter 26.8
mod spi_consts {
    pub mod cr {
        pub const SPIEN: u32 = 1 << 0;
        pub const SPIDIS: u32 = 1 << 1;
        pub const SWRST: u32 = 1 << 7;
        pub const FLUSHFIFO: u32 = 1 << 8;
        pub const LASTXFER: u32 = 1 << 24;
    }

    pub mod mr {
        pub const MSTR: u32 = 1 << 0;
        pub const PS: u32 = 1 << 1;
        pub const PCSDEC: u32 = 1 << 2;
        pub const MODFDIS: u32 = 1 << 4;
        pub const RXFIFOEN: u32 = 1 << 6;
        pub const LLB: u32 = 1 << 7;
        pub const PCS_MASK: u32 = 0b1111 << 16;
        pub const PCS0: u32 = 0b1110 << 16;
        pub const PCS1: u32 = 0b1101 << 16;
        pub const PCS2: u32 = 0b1011 << 16;
        pub const PCS3: u32 = 0b0111 << 16;
        pub const DLYBCS_MASK: u32 = 0xFF << 24;
    }

    pub mod rdr {
        pub const RD: u32 = 0xFFFF;
    }

    pub mod tdr {
        pub const TD: u32 = 0xFFFF;
        // PCSx masks from MR also apply here
        // LASTXFER from CR also applies here
    }

    pub mod sr {
        // These same bits are used in IDR, IER, and IMR.
        pub const RDRF: u32 = 1 << 0;
        pub const TDRE: u32 = 1 << 1;
        pub const MODF: u32 = 1 << 2;
        pub const OVRES: u32 = 1 << 3;
        pub const NSSR: u32 = 1 << 8;
        pub const TXEMPTY: u32 = 1 << 9;
        pub const UNDES: u32 = 1 << 10;

        // This only exists in the SR
        pub const SPIENS: u32 = 1 << 16;
    }

    // These bit masks apply to CSR0; CSR1, CSR2, CSR3
    pub mod csr {
        pub const CPOL: u32 = 1 << 0;
        pub const NCPHA: u32 = 1 << 1;
        pub const CSNAAT: u32 = 1 << 2;
        pub const CSAAT: u32 = 1 << 3;
        pub const BITS_MASK: u32 = 0x1111 << 4;
        pub const BITS8: u32 = 0b0000 << 4;
        pub const BITS9: u32 = 0b0001 << 4;
        pub const BITS10: u32 = 0b0010 << 4;
        pub const BITS11: u32 = 0b0011 << 4;
        pub const BITS12: u32 = 0b0100 << 4;
        pub const BITS13: u32 = 0b0101 << 4;
        pub const BITS14: u32 = 0b0110 << 4;
        pub const BITS15: u32 = 0b0111 << 4;
        pub const BITS16: u32 = 0b1000 << 4;
        pub const BITS4: u32 = 0b1001 << 4;
        pub const BITS5: u32 = 0b1010 << 4;
        pub const BITS6: u32 = 0b1011 << 4;
        pub const BITS7: u32 = 0b1100 << 4;
        pub const SCBR_MASK: u32 = 0xFF << 8;
        pub const DLYBS_MASK: u32 = 0xFF << 16;
        pub const DLYBCT_MASK: u32 = 0xFF << 24;
    }
}

const SPI_BASE: u32 = 0x40008000;

/// Values for selected peripherals
#[derive(Copy,Clone)]
pub enum Peripheral {
    Peripheral0,
    Peripheral1,
    Peripheral2,
    Peripheral3,
}

#[derive(Copy,Clone,PartialEq)]
pub enum SpiRole {
    SpiMaster,
    SpiSlave,
}

/// The SAM4L supports four peripherals.
pub struct Spi<'a> {
    registers: *mut SpiRegisters,
    client: Cell<Option<&'static SpiMasterClient>>,
    dma_read: Cell<Option<&'static DMAChannel>>,
    dma_write: Cell<Option<&'static DMAChannel>>,
    // keep track of which how many DMA transfers are pending to correctly
    // issue completion event only after both complete.
    transfers_in_progress: Cell<u8>,
    dma_length: Cell<usize>,

    // Slave client is distinct from master client
    slave_client: Cell<Option<&'static SpiSlaveClient>>,
    role: Cell<SpiRole>,

    baud_rate: Cell<u32>,

    callback_read_buffer: TakeCell<'static, [u8]>,
    callback_write_buffer: TakeCell<'static, [u8]>,
    callback_len: Cell<usize>,

    cm_enabled: Cell<bool>,
    return_params: Cell<bool>,
    clock_params: ClockParams,
    has_lock: Cell<bool>,
    next: ListLink<'a, ClockClient<'a>>,
}

pub static mut SPI: Spi = Spi::new();

impl<'a> Spi<'a> {
    /// Creates a new SPI object, with peripheral 0 selected
    pub const fn new() -> Spi<'a> {
        Spi {
            registers: SPI_BASE as *mut SpiRegisters,
            client: Cell::new(None),
            dma_read: Cell::new(None),
            dma_write: Cell::new(None),
            transfers_in_progress: Cell::new(0),
            dma_length: Cell::new(0),

            slave_client: Cell::new(None),
            role: Cell::new(SpiRole::SpiMaster),

            baud_rate: Cell::new(0),

            callback_read_buffer: TakeCell::empty(),
            callback_write_buffer: TakeCell::empty(),
            callback_len: Cell::new(0),

            cm_enabled: Cell::new(false),
            return_params: Cell::new(false),
            has_lock: Cell::new(false),
            //TODO: spi doesn't work with RC1M or RCFAST
            clock_params: ClockParams::new(0x00000010, 0xffffffff, 80000000, 1000000, 1000000),
            next: ListLink::empty(),
        }
    }

    fn init_as_role(&self, role: SpiRole) {

        self.role.set(role);

        self.enable_clock();
        self.spi_reset();

        if self.role.get() == SpiRole::SpiMaster {
            // Only need to set LASTXFER if we are master
            let regs: &SpiRegisters = unsafe { &*self.registers };
            regs.cr.set(spi_consts::cr::LASTXFER);
        }

        self.spi_set_mode(role);
        self.spi_disable_mode_fault_detect();
        self.spi_disable_loopback();

        self.spi_set_bits_per_transfer(spi_consts::csr::BITS8);

    }

    pub fn enable(&self) {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        //self.enable_clock();

        regs.cr.set(spi_consts::cr::SPIEN);

        if self.role.get() == SpiRole::SpiSlave {
            regs.ier.set(spi_consts::sr::NSSR); // Enable NSSR
        }
    }

    //Sofware reset of SPI, SPI is in slave mode after reset
    fn spi_reset(&self) {
        let regs: &SpiRegisters = unsafe { &*self.registers };
        regs.cr.set(spi_consts::cr::SWRST);
    }

    fn spi_set_mode(&self, role: SpiRole) {
        let regs: &SpiRegisters = unsafe { &*self.registers };
        let mut mode = regs.mr.get();
        match self.role.get() {
            SpiRole::SpiMaster => mode |= spi_consts::mr::MSTR,
            SpiRole::SpiSlave => mode &= !spi_consts::mr::MSTR,
        }
        regs.mr.set(mode);
    }

    // Disable mode fault detection (open drain outputs not supported)
    fn spi_disable_mode_fault_detect(&self){
        let regs: &SpiRegisters = unsafe { &*self.registers };
        let mut mode = regs.mr.get();
        mode |= spi_consts::mr::MODFDIS;
        regs.mr.set(mode);
    }

    fn spi_disable_loopback(&self){
        let regs: &SpiRegisters = unsafe { &*self.registers };
        let mut mode = regs.mr.get();
        mode &= !spi_consts::mr::LLB;
        regs.mr.set(mode);
    }

    fn spi_set_bits_per_transfer(&self, bit_rate: u32){
        let mut csr = self.read_active_csr();
        csr &= !spi_consts::csr::BITS_MASK;
        csr |= bit_rate;
        self.write_active_csr(csr);
    }

    pub fn disable(&self) {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        self.dma_read.get().map(|read| read.disable());
        self.dma_write.get().map(|write| write.disable());
        regs.cr.set(spi_consts::cr::SPIDIS);

        if self.role.get() == SpiRole::SpiSlave {
            regs.idr.set(spi_consts::sr::NSSR); // Disable NSSR
        }
    }

    /// Sets the approximate baud rate for the active peripheral,
    /// and return the actual baud rate set.
    ///
    /// Since the only supported baud rates are (system clock / n) where n
    /// is an integer from 1 to 255, the exact baud rate may not
    /// be available. In that case, the next lower baud rate will
    /// be selected.
    ///
    /// The lowest available baud rate is 188235 baud. If the
    /// requested rate is lower, 188235 baud will be selected.
    pub fn set_baud_rate(&self, rate: u32) -> u32 {
        // Main clock frequency
        let mut real_rate = rate;
        let clock = pm::get_system_frequency();

        self.baud_rate.set(rate);
        self.clock_params.min_frequency.set(rate);
        self.clock_params.thresh_frequency.set(rate);
        //TODO: bus clock could be further divided?
        self.clock_params.max_frequency.set(rate*255);

        if real_rate < 188235 {
            real_rate = 188235;
        }
        if real_rate > clock {
            real_rate = clock;
        }

        // Divide, rounding up to the nearest integer
        let scbr = (clock + real_rate - 1) / real_rate;

        if (scbr > 0) && (scbr <= 255) {
            let mut csr = self.read_active_csr();
            csr = (csr & !spi_consts::csr::SCBR_MASK) | ((scbr & 0xFF) << 8);
            self.write_active_csr(csr);
        }
        clock / scbr
    }

    pub fn get_baud_rate(&self) -> u32 {
        let clock = 48000000;
        let scbr = (self.read_active_csr() & spi_consts::csr::SCBR_MASK) >> 8;
        clock / scbr
    }

    fn set_clock(&self, polarity: ClockPolarity) {
        let mut csr = self.read_active_csr();
        match polarity {
            ClockPolarity::IdleHigh => csr |= spi_consts::csr::CPOL,
            ClockPolarity::IdleLow => csr &= !spi_consts::csr::CPOL,
        };
        self.write_active_csr(csr);
    }

    fn get_clock(&self) -> ClockPolarity {
        let csr = self.read_active_csr();
        let polarity = csr & spi_consts::csr::CPOL;
        match polarity {
            0 => ClockPolarity::IdleLow,
            _ => ClockPolarity::IdleHigh,
        }
    }

    fn set_phase(&self, phase: ClockPhase) {
        let mut csr = self.read_active_csr();
        match phase {
            ClockPhase::SampleLeading => csr |= spi_consts::csr::NCPHA,
            ClockPhase::SampleTrailing => csr &= !spi_consts::csr::NCPHA,
        };
        self.write_active_csr(csr);
    }

    fn get_phase(&self) -> ClockPhase {
        let csr = self.read_active_csr();
        let phase = csr & spi_consts::csr::NCPHA;
        match phase {
            0 => ClockPhase::SampleTrailing,
            _ => ClockPhase::SampleLeading,
        }
    }

    pub fn set_active_peripheral(&self, peripheral: Peripheral) {
        // Slave cannot set active peripheral
        if self.role.get() == SpiRole::SpiMaster {
            let regs: &SpiRegisters = unsafe { &*self.registers };
            let peripheral_number: u32 = match peripheral {
                Peripheral::Peripheral0 => spi_consts::mr::PCS0,
                Peripheral::Peripheral1 => spi_consts::mr::PCS1,
                Peripheral::Peripheral2 => spi_consts::mr::PCS2,
                Peripheral::Peripheral3 => spi_consts::mr::PCS3,
            };
            let mut mr = regs.mr.get();
            mr = (mr & !spi_consts::mr::PCS_MASK) | peripheral_number;
            regs.mr.set(mr);
        }
    }

    /// Returns the currently active peripheral
    pub fn get_active_peripheral(&self) -> Peripheral {
        if self.role.get() == SpiRole::SpiMaster {
            let regs: &SpiRegisters = unsafe { &*self.registers };

            let mr = regs.mr.get();
            let peripheral_number = mr & (spi_consts::mr::PCS_MASK);

            match peripheral_number {
                spi_consts::mr::PCS0 => Peripheral::Peripheral0,
                spi_consts::mr::PCS1 => Peripheral::Peripheral1,
                spi_consts::mr::PCS2 => Peripheral::Peripheral2,
                spi_consts::mr::PCS3 => Peripheral::Peripheral3,
                _ => {
                    // Invalid configuration
                    Peripheral::Peripheral0
                }
            }
        } else {
            // The active peripheral is always 0 in slave mode
            Peripheral::Peripheral0
        }
    }

    /// Returns the value of CSR0, CSR1, CSR2, or CSR3,
    /// whichever corresponds to the active peripheral
    fn read_active_csr(&self) -> u32 {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        match self.get_active_peripheral() {
            Peripheral::Peripheral0 => regs.csr0.get(),
            Peripheral::Peripheral1 => regs.csr1.get(),
            Peripheral::Peripheral2 => regs.csr2.get(),
            Peripheral::Peripheral3 => regs.csr3.get(),
        }
    }
    /// Sets the Chip Select Register (CSR) of the active peripheral
    /// (CSR0, CSR1, CSR2, or CSR3).
    fn write_active_csr(&self, value: u32) {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        match self.get_active_peripheral() {
            Peripheral::Peripheral0 => regs.csr0.set(value),
            Peripheral::Peripheral1 => regs.csr1.set(value),
            Peripheral::Peripheral2 => regs.csr2.set(value),
            Peripheral::Peripheral3 => regs.csr3.set(value),
        };
    }

    /// Set the DMA channels used for reading and writing.
    pub fn set_dma(&mut self, read: &'static DMAChannel, write: &'static DMAChannel) {
        self.dma_read.set(Some(read));
        self.dma_write.set(Some(write));
    }

    fn enable_clock(&self) {
        unsafe {
            pm::enable_clock(pm::Clock::PBA(pm::PBAClock::SPI));
        }
    }

    pub fn handle_interrupt(&self) {
        let regs: &SpiRegisters = unsafe { &*self.registers };
        let sr = regs.sr.get();

        self.slave_client.get().map(|client| {
            if (sr & spi_consts::sr::NSSR) != 0 {
                // NSSR
                client.chip_selected()
            }
            // TODO: Do we want to support byte-level interrupts too?
            // They currently conflict with DMA.
        });
    }

    /// Asynchronous buffer read/write of SPI.
    /// returns `SUCCESS` if operation starts (will receive callback through SpiMasterClient),
    /// returns `EBUSY` if the operation does not start.
    // The write buffer has to be mutable because it's passed back to
    // the caller, and the caller may want to be able write into it.
    fn read_write_bytes(&self,
                        write_buffer: Option<&'static mut [u8]>,
                        read_buffer: Option<&'static mut [u8]>,
                        len: usize)
                        -> ReturnCode {
        self.enable();

        if write_buffer.is_none() && read_buffer.is_none() {
            return ReturnCode::SUCCESS;
        }

        let mut opt_len = None;
        write_buffer.as_ref().map(|buf| opt_len = Some(buf.len()));
        read_buffer.as_ref().map(|buf| {
            let min_len = opt_len.map_or(buf.len(), |old_len| cmp::min(old_len, buf.len()));
            opt_len = Some(min_len);
        });

        let count = cmp::min(opt_len.unwrap_or(0), len);
        self.dma_length.set(count);

        // Reset the number of transfers in progress. This is incremented
        // depending on the presence of the read/write below
        self.transfers_in_progress.set(0);

        self.callback_len.set(count);
        match read_buffer {
            Some(buf) => self.callback_read_buffer.put(Some(buf)),
            None => self.callback_read_buffer.put(None),
        }
        match write_buffer {
            Some(buf) => self.callback_write_buffer.put(Some(buf)),
            None => self.callback_write_buffer.put(None),
        }

        if self.cm_enabled.get() && !self.has_lock.get() {
            self.return_params.set(true);
            unsafe {
                clock_pm::CM.clock_change();
            }
        }
        else {
            self.read_write_callback();
        }
        ReturnCode::SUCCESS
    }

    fn read_write_callback(&self) {

        // The ordering of these operations matters.
        // For transfers 4 bytes or longer, this will work as expected.
        // For shorter transfers, the first byte will be missing.
        self.callback_write_buffer.take().map(|wbuf| {
            self.transfers_in_progress.set(self.transfers_in_progress.get() + 1);
            self.dma_write.get().map(move |write| {
                write.enable();
                write.do_xfer(DMAPeripheral::SPI_TX, wbuf, self.callback_len.get());
            });
        });

        // Only setup the RX channel if we were passed a read_buffer inside
        // of the option. `map()` checks this for us.
        self.callback_read_buffer.take().map(|rbuf| {
            self.transfers_in_progress.set(self.transfers_in_progress.get() + 1);
            self.dma_read.get().map(move |read| {
                read.enable();
                read.do_xfer(DMAPeripheral::SPI_RX, rbuf, self.callback_len.get());
            });
        });
    }
}

impl<'a> spi::SpiMaster for Spi<'a> {
    type ChipSelect = u8;

    fn set_client(&self, client: &'static SpiMasterClient) {
        self.client.set(Some(client));
    }

    /// By default, initialize SPI to operate at 40KHz, clock is
    /// idle on low, and sample on the leading edge.
    fn init(&self) {
        self.init_as_role(SpiRole::SpiMaster);
    }

    fn is_busy(&self) -> bool {
        self.transfers_in_progress.get() != 0
    }

    /// Write a byte to the SPI and discard the read; if an
    /// asynchronous operation is outstanding, do nothing.
    fn write_byte(&self, out_byte: u8) {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        let tdr = (out_byte as u32) & spi_consts::tdr::TD;
        // Wait for data to leave TDR and enter serializer, so TDR is free
        // for this next byte
        while (regs.sr.get() & spi_consts::sr::TDRE) == 0 {}
        regs.tdr.set(tdr);
    }

    /// Write 0 to the SPI and return the read; if an
    /// asynchronous operation is outstanding, do nothing.
    fn read_byte(&self) -> u8 {
        self.read_write_byte(0)
    }

    /// Write a byte to the SPI and return the read; if an
    /// asynchronous operation is outstanding, do nothing.
    fn read_write_byte(&self, val: u8) -> u8 {
        let regs: &SpiRegisters = unsafe { &*self.registers };

        self.write_byte(val);
        // Wait for receive data register full
        while (regs.sr.get() & spi_consts::sr::RDRF) == 0 {}
        // Return read value
        regs.rdr.get() as u8
    }

    /// Asynchronous buffer read/write of SPI.
    /// write_buffer must  be Some; read_buffer may be None;
    /// if read_buffer is Some, then length of read/write is the
    /// minimum of two buffer lengths; returns `SUCCESS` if operation
    /// starts (will receive callback through SpiMasterClient), returns
    /// `EBUSY` if the operation does not start.
    // The write buffer has to be mutable because it's passed back to
    // the caller, and the caller may want to be able write into it.
    fn read_write_bytes(&self,
                        write_buffer: &'static mut [u8],
                        read_buffer: Option<&'static mut [u8]>,
                        len: usize)
                        -> ReturnCode {
        //self.enable();

        // If busy, don't start.
        if self.is_busy() {
            return ReturnCode::EBUSY;
        }

        self.read_write_bytes(Some(write_buffer), read_buffer, len)
    }

    fn set_rate(&self, rate: u32) -> u32 {
        self.set_baud_rate(rate)
    }

    fn get_rate(&self) -> u32 {
        self.get_baud_rate()
    }

    fn set_clock(&self, polarity: ClockPolarity) {
        self.set_clock(polarity);
    }

    fn get_clock(&self) -> ClockPolarity {
        self.get_clock()
    }

    fn set_phase(&self, phase: ClockPhase) {
        self.set_phase(phase);
    }

    fn get_phase(&self) -> ClockPhase {
        self.get_phase()
    }

    fn hold_low(&self) {
        let mut csr = self.read_active_csr();
        csr |= spi_consts::csr::CSAAT;
        self.write_active_csr(csr);
    }

    fn release_low(&self) {
        let mut csr = self.read_active_csr();
        csr &= !spi_consts::csr::CSAAT;
        self.write_active_csr(csr);
    }

    fn specify_chip_select(&self, cs: Self::ChipSelect) {
        let peripheral_number = match cs {
            0 => Peripheral::Peripheral0,
            1 => Peripheral::Peripheral1,
            2 => Peripheral::Peripheral2,
            3 => Peripheral::Peripheral3,
            _ => Peripheral::Peripheral0,
        };
        self.set_active_peripheral(peripheral_number);
    }
}

impl<'a> spi::SpiSlave for Spi<'a> {
    // Set to None to disable the whole thing
    fn set_client(&self, client: Option<&'static SpiSlaveClient>) {
        self.slave_client.set(client);
    }

    fn has_client(&self) -> bool {
        self.slave_client.get().is_some()
    }

    fn init(&self) {
        self.init_as_role(SpiRole::SpiSlave);
    }

    /// This sets the value in the TDR register, to be sent as soon as the
    /// chip select pin is low.
    fn set_write_byte(&self, write_byte: u8) {
        let regs: &SpiRegisters = unsafe { &*self.registers };
        regs.tdr.set(write_byte as u32);
    }

    fn read_write_bytes(&self,
                        write_buffer: Option<&'static mut [u8]>,
                        read_buffer: Option<&'static mut [u8]>,
                        len: usize)
                        -> ReturnCode {
        self.read_write_bytes(write_buffer, read_buffer, len)
    }

    fn set_clock(&self, polarity: ClockPolarity) {
        self.set_clock(polarity);
    }

    fn get_clock(&self) -> ClockPolarity {
        self.get_clock()
    }

    fn set_phase(&self, phase: ClockPhase) {
        self.set_phase(phase);
    }

    fn get_phase(&self) -> ClockPhase {
        self.get_phase()
    }
}

impl<'a> DMAClient for Spi<'a>{
    fn xfer_done(&self, _pid: DMAPeripheral) {
        // Only callback that the transfer is done if either:
        // 1) The transfer was TX only and TX finished
        // 2) The transfer was TX and RX, in that case wait for both of them to complete. Although
        //    both transactions happen simultaneously over the wire, the DMA may not finish copying
        //    data over to/from the controller at the same time, so we don't want to abort
        //    prematurely.

        self.transfers_in_progress.set(self.transfers_in_progress.get() - 1);

        if self.transfers_in_progress.get() == 0 {
            let txbuf = self.dma_write.get().map_or(None, |dma| {
                let buf = dma.abort_xfer();
                dma.disable();
                buf
            });

            let rxbuf = self.dma_read.get().map_or(None, |dma| {
                let buf = dma.abort_xfer();
                dma.disable();
                buf
            });

            let len = self.dma_length.get();
            self.dma_length.set(0);

            match self.role.get() {
                SpiRole::SpiMaster => {
                    self.client
                        .get()
                        .map(|cb| {
                            txbuf.map(|txbuf| {
                                cb.read_write_done(txbuf, rxbuf, len);
                            });
                        });
                }
                SpiRole::SpiSlave => {
                    self.slave_client
                        .get()
                        .map(|cb| { cb.read_write_done(txbuf, rxbuf, len); });
                }
            }

            self.return_params.set(false);
        }
    }
}

impl<'a> hil::clock_pm::ClockClient<'a> for Spi<'a> {
    fn enable_cm(&self) {
        self.cm_enabled.set(true);
    }

    fn clock_updated(&self, clock_changed: bool) {
        if clock_changed {
            self.set_baud_rate(self.baud_rate.get());
        }

        if !self.has_lock.get() {
            unsafe {
                self.has_lock.set(clock_pm::CM.lock()); 
            }
            if !self.has_lock.get() {
                return;
            }
        }
        self.return_params.set(false);

        self.read_write_callback();
    }

    fn get_params(&self) -> Option<&ClockParams> {
        if self.return_params.get() {
            return Some(&self.clock_params);
        }
        None
    }

    fn next_link(&'a self) -> &'a ListLink<'a, ClockClient<'a> + 'a> {
        &self.next
    }
}

