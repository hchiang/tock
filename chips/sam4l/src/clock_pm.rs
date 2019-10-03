use kernel::hil::clock_pm::{SetClock};
use crate::pm;
use kernel::common::registers::{ReadWrite};
use kernel::common::StaticRef;
use kernel::debug;

struct DWTRegisters {
    ctrl: ReadWrite<u32>,
    cycnt: ReadWrite<u32>,
}

struct DBGRegisters {
    demcr: ReadWrite<u32>,
}

const DWT: StaticRef<DWTRegisters> = unsafe { StaticRef::new(0xE0001000 as *const _) };
const DEMCR: StaticRef<ReadWrite<u32>> = unsafe { StaticRef::new(0xE000EDFC as *const _) };

pub struct ImixClockManager{
}

impl ImixClockManager {

    pub const fn new() -> ImixClockManager {
        ImixClockManager {}
    }

    fn convert_to_clock(&self, clock: u32)->pm::SystemClockSource{
        //Roughly ordered in terms of least to most power consumption
        let mut system_clock = pm::SystemClockSource::RcsysAt115kHz;
        match clock {
            1 => system_clock = pm::SystemClockSource::RC1M,
            2 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency4MHz},
            3 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency8MHz},
            4 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency12MHz},
            5 => system_clock = pm::SystemClockSource::ExternalOscillator{
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            6 => system_clock = pm::SystemClockSource::DfllRc32kAt48MHz,
            7 => system_clock = pm::SystemClockSource::PllExternalOscillatorAt48MHz{ 
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            8 => system_clock = pm::SystemClockSource::RC80M,
            9 => system_clock = pm::SystemClockSource::RcsysAt115kHz,
            _ => system_clock = pm::SystemClockSource::DfllRc32kAt48MHz,
        }
        return system_clock;
    }


}


//Allows userland code to change the clock
impl SetClock for ImixClockManager {
    fn set_clock(&self, clock: u32) {
        let system_clock = self.convert_to_clock(clock);
        unsafe {
            pm::PM.change_system_clock(system_clock);
        }
    }
    fn start_timer(&self) {
        DEMCR.set(DEMCR.get() | 0x01000000);
        DWT.cycnt.set(0); // reset the counter
        DWT.ctrl.set(1);
    }
    fn get_timer(&self) {
        debug!("Timer {}", DWT.cycnt.get());
    }
}

