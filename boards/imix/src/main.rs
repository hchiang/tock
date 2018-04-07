#![no_std]
#![no_main]
#![feature(asm, const_fn, lang_items, compiler_builtins_lib, const_cell_new)]

extern crate capsules;
extern crate compiler_builtins;
#[allow(unused_imports)]
#[macro_use(debug, debug_gpio, static_init)]
extern crate kernel;
extern crate sam4l;

use capsules::alarm::AlarmDriver;
use capsules::virtual_alarm::{MuxAlarm, VirtualMuxAlarm};
use capsules::virtual_spi::{MuxSpiMaster, VirtualSpiMasterDevice};
use kernel::hil;
use kernel::hil::Controller;
use kernel::hil::spi::SpiMaster;
use kernel::hil::clock_pm::ClockManager;

#[macro_use]
pub mod io;

// Unit Tests for drivers.
#[allow(dead_code)]
mod spi_dummy;

#[allow(dead_code)]
mod power;

// State for loading apps.

const NUM_PROCS: usize = 2;

// how should the kernel respond when a process faults
const FAULT_RESPONSE: kernel::process::FaultResponse = kernel::process::FaultResponse::Panic;

#[link_section = ".app_memory"]
static mut APP_MEMORY: [u8; 16384] = [0; 16384];

static mut PROCESSES: [Option<kernel::Process<'static>>; NUM_PROCS] = [None, None];

struct Imix {
    console: &'static capsules::console::Console<'static, sam4l::usart::USART<'static>>,
    spi: &'static capsules::spi::Spi<'static, VirtualSpiMasterDevice<'static, sam4l::spi::Spi<'static>>>,
    adc: &'static capsules::adc::Adc<'static, sam4l::adc::Adc<'static>>,
    flash_driver: &'static capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
    alarm: &'static AlarmDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast<'static>>>,
    ipc: kernel::ipc::IPC,
}

impl kernel::Platform for Imix {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
    where
        F: FnOnce(Option<&kernel::Driver>) -> R,
    {
        match driver_num {
            capsules::console::DRIVER_NUM => f(Some(self.console)),
            capsules::alarm::DRIVER_NUM => f(Some(self.alarm)),
            capsules::spi::DRIVER_NUM => f(Some(self.spi)),
            capsules::adc::DRIVER_NUM => f(Some(self.adc)),
            kernel::ipc::DRIVER_NUM => f(Some(&self.ipc)),
            27 => f(Some(self.flash_driver)),
            _ => f(None),
        }
    }
}

unsafe fn set_pin_primary_functions() {
    use sam4l::gpio::{PA, PB, PC};
    use sam4l::gpio::PeripheralFunction::{A, B, C, E};

    // Right column: Imix pin name
    // Left  column: SAM4L peripheral function
    PA[04].configure(Some(A)); // AD0         --  ADCIFE AD0
    PA[05].configure(Some(A)); // AD1         --  ADCIFE AD1
    PA[06].configure(Some(C)); // EXTINT1     --  EIC EXTINT1
    PA[07].configure(Some(A)); // AD1         --  ADCIFE AD2
    PA[08].configure(None); //... RF233 IRQ   --  GPIO pin
    PA[09].configure(None); //... RF233 RST   --  GPIO pin
    PA[10].configure(None); //... RF233 SLP   --  GPIO pin
    PA[13].configure(None); //... TRNG EN     --  GPIO pin
    PA[14].configure(None); //... TRNG_OUT    --  GPIO pin
    PA[17].configure(None); //... NRF INT     -- GPIO pin
    PA[18].configure(Some(A)); // NRF CLK     -- USART2_CLK
    PA[20].configure(None); //... D8          -- GPIO pin
    PA[21].configure(Some(E)); // TWI2 SDA    -- TWIM2_SDA
    PA[22].configure(Some(E)); // TWI2 SCL    --  TWIM2 TWCK
    PA[25].configure(Some(A)); // USB_N       --  USB DM
    PA[26].configure(Some(A)); // USB_P       --  USB DP
    PB[00].configure(Some(A)); // TWI1_SDA    --  TWIMS1 TWD
    PB[01].configure(Some(A)); // TWI1_SCL    --  TWIMS1 TWCK
    PB[02].configure(Some(A)); // AD3         --  ADCIFE AD3
    PB[03].configure(Some(A)); // AD4         --  ADCIFE AD4
    PB[04].configure(Some(A)); // AD5         --  ADCIFE AD5
    PB[05].configure(Some(A)); // VHIGHSAMPLE --  ADCIFE AD6
    PB[06].configure(Some(A)); // RTS3        --  USART3 RTS
    PB[07].configure(None); //... NRF RESET   --  GPIO
    PB[09].configure(Some(A)); // RX3         --  USART3 RX
    PB[10].configure(Some(A)); // TX3         --  USART3 TX
    PB[11].configure(Some(A)); // CTS0        --  USART0 CTS
    PB[12].configure(Some(A)); // RTS0        --  USART0 RTS
    PB[13].configure(Some(A)); // CLK0        --  USART0 CLK
    PB[14].configure(Some(A)); // RX0         --  USART0 RX
    PB[15].configure(Some(A)); // TX0         --  USART0 TX
    PC[00].configure(Some(A)); // CS2         --  SPI NPCS2
    PC[01].configure(Some(A)); // CS3 (RF233) --  SPI NPCS3
    PC[02].configure(Some(A)); // CS1         --  SPI NPCS1
    PC[03].configure(Some(A)); // CS0         --  SPI NPCS0
    PC[04].configure(Some(A)); // MISO        --  SPI MISO
    PC[05].configure(Some(A)); // MOSI        --  SPI MOSI
    PC[06].configure(Some(A)); // SCK         --  SPI CLK
    PC[07].configure(Some(B)); // RTS2 (BLE)  -- USART2_RTS
    PC[08].configure(Some(E)); // CTS2 (BLE)  -- USART2_CTS
    PC[09].configure(None); //... NRF GPIO    -- GPIO
    PC[10].configure(None); //... USER LED    -- GPIO
    PC[11].configure(Some(B)); // RX2 (BLE)   -- USART2_RX
    PC[12].configure(Some(B)); // TX2 (BLE)   -- USART2_TX
    PC[13].configure(None); //... ACC_INT1    -- GPIO
    PC[14].configure(None); //... ACC_INT2    -- GPIO
    PC[16].configure(None); //... SENSE_PWR   --  GPIO pin
    PC[17].configure(None); //... NRF_PWR     --  GPIO pin
    PC[18].configure(None); //... RF233_PWR   --  GPIO pin
    PC[19].configure(None); //... TRNG_PWR    -- GPIO Pin
    PC[22].configure(None); //... KERNEL LED  -- GPIO Pin
    PC[24].configure(None); //... USER_BTN    -- GPIO Pin
    PC[25].configure(Some(B)); // LI_INT      --  EIC EXTINT2
    PC[26].configure(None); //... D7          -- GPIO Pin
    PC[27].configure(None); //... D6          -- GPIO Pin
    PC[28].configure(None); //... D5          -- GPIO Pin
    PC[29].configure(None); //... D4          -- GPIO Pin
    PC[30].configure(None); //... D3          -- GPIO Pin
    PC[31].configure(None); //... D2          -- GPIO Pin
}

#[no_mangle]
pub unsafe fn reset_handler() {
    sam4l::init();

    //sam4l::pm::PM.setup_system_clock(sam4l::pm::SystemClockSource::DfllRc32kAt48MHz);
    //sam4l::pm::PM.setup_system_clock(sam4l::pm::SystemClockSource::PllExternalOscillatorAt48MHz {
    sam4l::pm::PM.setup_system_clock(sam4l::pm::SystemClockSource::ExternalOscillator{
        frequency: sam4l::pm::OscillatorFrequency::Frequency16MHz,
        startup_mode: sam4l::pm::OscillatorStartup::FastStart,
    });

    // Source 32Khz and 1Khz clocks from RC23K (SAM4L Datasheet 11.6.8)
    sam4l::bpm::set_ck32source(sam4l::bpm::CK32Source::RC32K);

    //set_pin_primary_functions();

    //kernel::debug::assign_gpios(Some(&sam4l::gpio::PC[26]),
    //                            Some(&sam4l::gpio::PC[27]),
    //                            Some(&sam4l::gpio::PC[28]));
    //debug_gpio!(0, make_output);
    //debug_gpio!(0, clear);
    //debug_gpio!(1, make_output);
    //debug_gpio!(1, clear);
    //debug_gpio!(2, make_output);
    //debug_gpio!(2, clear);

    // # CONSOLE

    let console = static_init!(
        capsules::console::Console<sam4l::usart::USART<'static>>,
        capsules::console::Console::new(
            &sam4l::usart::USART3,
            115200,
            &mut capsules::console::WRITE_BUF,
            &mut capsules::console::READ_BUF,
            kernel::Grant::create()
        )
    );
    hil::uart::UART::set_client(&sam4l::usart::USART3, console);
    // Debug statments have to occur after register or no output
    //sam4l::clock_pm::CM.register(&sam4l::usart::USART3);
    console.initialize();

    // Attach the kernel debug interface to this console
    let kc = static_init!(capsules::console::App, capsules::console::App::default());
    kernel::debug::assign_console_driver(Some(console), kc);

    // # TIMER

    let ast = &sam4l::ast::AST;

    let mux_alarm = static_init!(
        MuxAlarm<'static, sam4l::ast::Ast>,
        MuxAlarm::new(&sam4l::ast::AST)
    );
    ast.configure(mux_alarm);

    let virtual_alarm1 = static_init!(
        VirtualMuxAlarm<'static, sam4l::ast::Ast>,
        VirtualMuxAlarm::new(mux_alarm)
    );
    let alarm = static_init!(
        AlarmDriver<'static, VirtualMuxAlarm<'static, sam4l::ast::Ast>>,
        AlarmDriver::new(virtual_alarm1, kernel::Grant::create())
    );
    virtual_alarm1.set_client(alarm);

    // Setup ADC
    let adc_channels = static_init!(
        [&'static sam4l::adc::AdcChannel; 6],
        [
            &sam4l::adc::CHANNEL_AD1, // AD0
            &sam4l::adc::CHANNEL_AD2, // AD1
            &sam4l::adc::CHANNEL_AD3, // AD2
            &sam4l::adc::CHANNEL_AD4, // AD3
            &sam4l::adc::CHANNEL_AD5, // AD4
            &sam4l::adc::CHANNEL_AD6  // AD5
        ]
    );
    let adc = static_init!(
        capsules::adc::Adc<'static, sam4l::adc::Adc>,
        capsules::adc::Adc::new(
            &mut sam4l::adc::ADC0,
            adc_channels,
            &mut capsules::adc::ADC_BUFFER1,
            &mut capsules::adc::ADC_BUFFER2,
            &mut capsules::adc::ADC_BUFFER3
        )
    );
    sam4l::adc::ADC0.set_client(adc);
    //sam4l::clock_pm::CM.register(&sam4l::adc::ADC0);

    // Set up an SPI MUX, so there can be multiple clients
    let mux_spi = static_init!(
        MuxSpiMaster<'static, sam4l::spi::Spi<'static>>,
        MuxSpiMaster::new(&sam4l::spi::SPI)
    );
    sam4l::spi::SPI.set_client(mux_spi);
    sam4l::spi::SPI.init();
    //sam4l::spi::SPI.enable();
    //sam4l::clock_pm::CM.register(&sam4l::spi::SPI);

    // Create a virtualized client for SPI system call interface,
    // then the system call capsule
    let syscall_spi_device = static_init!(
        VirtualSpiMasterDevice<'static, sam4l::spi::Spi<'static>>,
        VirtualSpiMasterDevice::new(mux_spi, 3)
    );

    // Create the SPI systemc call capsule, passing the client
    let spi_syscalls = static_init!(
        capsules::spi::Spi<'static, VirtualSpiMasterDevice<'static, sam4l::spi::Spi<'static>>>,
        capsules::spi::Spi::new(syscall_spi_device)
    );

    // System call capsule requires static buffers so it can
    // copy from application slices to DMA
    static mut SPI_READ_BUF: [u8; 64] = [0; 64];
    static mut SPI_WRITE_BUF: [u8; 64] = [0; 64];
    spi_syscalls.config_buffers(&mut SPI_READ_BUF, &mut SPI_WRITE_BUF);
    syscall_spi_device.set_client(spi_syscalls);

    sam4l::flashcalw::FLASH_CONTROLLER.configure();
    pub static mut FLASH_PAGEBUFFER: sam4l::flashcalw::Sam4lPage = sam4l::flashcalw::Sam4lPage::new();
    let nv_to_page = static_init!(
        capsules::nonvolatile_to_pages::NonvolatileToPages<'static, sam4l::flashcalw::FLASHCALW>,
        capsules::nonvolatile_to_pages::NonvolatileToPages::new(
            &mut sam4l::flashcalw::FLASH_CONTROLLER,
            &mut FLASH_PAGEBUFFER));
    hil::flash::HasClient::set_client(&sam4l::flashcalw::FLASH_CONTROLLER, nv_to_page);
    //sam4l::clock_pm::CM.register(&sam4l::flashcalw::FLASH_CONTROLLER);

    let flash_driver = static_init!(
        capsules::nonvolatile_storage_driver::NonvolatileStorage<'static>,
        capsules::nonvolatile_storage_driver::NonvolatileStorage::new(
            nv_to_page, kernel::Grant::create(),
            0x60000, // Start address for userspace accessible region
            0x20000, // Length of userspace accessible region
            0,       // Start address of kernel accessible region
            0,       // Length of kernel accessible region
            &mut capsules::nonvolatile_storage_driver::BUFFER));
    hil::nonvolatile_storage::NonvolatileStorage::set_client(nv_to_page, flash_driver);

    let imix = Imix {
        console: console,
        alarm: alarm,
        adc: adc,
        spi: spi_syscalls,
        flash_driver: flash_driver,
        ipc: kernel::ipc::IPC::new(),
    };

    let mut chip = sam4l::chip::Sam4l::new();

    debug!("Initialization complete. Entering main loop");
    extern "C" {
        /// Beginning of the ROM region containing app images.
        static _sapps: u8;
    }
    kernel::process::load_processes(
        &_sapps as *const u8,
        &mut APP_MEMORY,
        &mut PROCESSES,
        FAULT_RESPONSE,
    );

    kernel::main(&imix, &mut chip, &mut PROCESSES, &imix.ipc);
}
