//! Components for SPI.
//!
//! This provides three components.
//!
//! 1. `SpiMuxComponent` provides a virtualization layer for a SPI bus.
//!
//! 2. `SpiSyscallComponent` provides a system call interface to SPI.
//!
//! 3. `SpiComponent` provides a virtualized client to the SPI bus.
//!
//! `SpiSyscallComponent` is used for processes, while `SpiComponent` is used
//! for kernel capsules that need access to the SPI bus.
//!
//! Usage
//! -----
//! ```rust
//! let mux_spi = components::spi::SpiMuxComponent::new(&sam4l::spi::SPI).finalize(
//!     components::spi_mux_component_helper!(sam4l::spi::SpiHw));
//! let spi_syscalls = SpiSyscallComponent::new(mux_spi, 3).finalize(
//!     components::spi_syscalls_component_helper!(sam4l::spi::SpiHw));
//! let rf233_spi = SpiComponent::new(mux_spi, 3).finalize(
//!     components::spi_component_helper!(sam4l::spi::SpiHw));
//! ```

// Author: Philip Levis <pal@cs.stanford.edu>
// Last modified: 6/20/2018

#![allow(dead_code)] // Components are intended to be conditionally included

use capsules::spi::Spi;
use capsules::virtual_spi::{MuxSpiMaster, VirtualSpiMasterDevice};
use kernel::component::Component;
use kernel::hil::spi;

use crate::static_init_half;

// Setup static space for the objects.
#[macro_export]
macro_rules! spi_mux_component_helper {
    ($S:ty) => {{
        static mut BUF: Option<MuxSpiMaster<'static, $S>> = None;
        &mut BUF
    };};
}

#[macro_export]
macro_rules! spi_syscall_component_helper {
    ($S:ty) => {{
        use capsules::spi::Spi;
        static mut BUF1: Option<VirtualSpiMasterDevice<'static, $S>> = None;
        static mut BUF2: Option<Spi<'static, VirtualSpiMasterDevice<'static, $S>>> = None;
        (&mut BUF1, &mut BUF2)
    };};
}

#[macro_export]
macro_rules! spi_component_helper {
    ($S:ty) => {{
        static mut BUF: Option<VirtualSpiMasterDevice<'static, $S>> = None;
        &mut BUF
    };};
}

pub struct SpiMuxComponent<S: 'static + spi::SpiMaster> {
    spi: &'static S,
}

pub struct SpiSyscallComponent<S: 'static + spi::SpiMaster> {
    spi_mux: &'static MuxSpiMaster<'static, S>,
    chip_select: S::ChipSelect,
}

pub struct SpiComponent<S: 'static + spi::SpiMaster> {
    spi_mux: &'static MuxSpiMaster<'static, S>,
    chip_select: S::ChipSelect,
}

impl<S: 'static + spi::SpiMaster> SpiMuxComponent<S> {
    pub fn new(spi: &'static S) -> Self {
        SpiMuxComponent { spi: spi }
    }
}

impl<S: 'static + spi::SpiMaster> Component for SpiMuxComponent<S> {
    type StaticInput = &'static mut Option<MuxSpiMaster<'static, S>>;
    type Output = &'static MuxSpiMaster<'static, S>;

    unsafe fn finalize(&mut self, static_buffer: Self::StaticInput) -> Self::Output {
        let mux_spi = static_init_half!(
            static_buffer,
            MuxSpiMaster<'static, S>,
            MuxSpiMaster::new(self.spi)
        );

        self.spi.set_client(mux_spi);
        self.spi.init();

        mux_spi
    }
}

impl<S: 'static + spi::SpiMaster> SpiSyscallComponent<S> {
    pub fn new(mux: &'static MuxSpiMaster<'static, S>, chip_select: S::ChipSelect) -> Self {
        SpiSyscallComponent {
            spi_mux: mux,
            chip_select: chip_select,
        }
    }
}

impl<S: 'static + spi::SpiMaster> Component for SpiSyscallComponent<S> {
    type StaticInput = (
        &'static mut Option<VirtualSpiMasterDevice<'static, S>>,
        &'static mut Option<Spi<'static, VirtualSpiMasterDevice<'static, S>>>,
    );
    type Output = &'static Spi<'static, VirtualSpiMasterDevice<'static, S>>;

    unsafe fn finalize(&mut self, static_buffer: Self::StaticInput) -> Self::Output {
        let syscall_spi_device = static_init_half!(
            static_buffer.0,
            VirtualSpiMasterDevice<'static, S>,
            VirtualSpiMasterDevice::new(self.spi_mux, self.chip_select)
        );

        let spi_syscalls = static_init_half!(
            static_buffer.1,
            Spi<'static, VirtualSpiMasterDevice<'static, S>>,
            Spi::new(syscall_spi_device)
        );

        static mut SPI_READ_BUF: [u8; 1024] = [0; 1024];
        static mut SPI_WRITE_BUF: [u8; 1024] = [0; 1024];

        spi_syscalls.config_buffers(&mut SPI_READ_BUF, &mut SPI_WRITE_BUF);
        syscall_spi_device.set_client(spi_syscalls);

        spi_syscalls
    }
}

impl<S: 'static + spi::SpiMaster> SpiComponent<S> {
    pub fn new(mux: &'static MuxSpiMaster<'static, S>, chip_select: S::ChipSelect) -> Self {
        SpiComponent {
            spi_mux: mux,
            chip_select: chip_select,
        }
    }
}

impl<S: 'static + spi::SpiMaster> Component for SpiComponent<S> {
    type StaticInput = &'static mut Option<VirtualSpiMasterDevice<'static, S>>;
    type Output = &'static VirtualSpiMasterDevice<'static, S>;

    unsafe fn finalize(&mut self, static_buffer: Self::StaticInput) -> Self::Output {
        let spi_device = static_init_half!(
            static_buffer,
            VirtualSpiMasterDevice<'static, S>,
            VirtualSpiMasterDevice::new(self.spi_mux, self.chip_select)
        );

        spi_device
    }
}
