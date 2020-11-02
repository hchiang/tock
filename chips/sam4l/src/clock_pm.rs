use kernel::hil::clock_pm::*;
use crate::pm;
use cortexm4;

const RCSYS: u32        = 0x001; 
const RC1M: u32         = 0x002; 
const RCFAST4M: u32     = 0x004; 
const RCFAST8M: u32     = 0x008;    
const RCFAST12M: u32    = 0x010; 
const EXTOSC: u32       = 0x020; 
const RC80M: u32        = 0x040;
const PLL: u32          = 0x080; 
const DFLL: u32         = 0x100; 
const ALL_CLOCKS: u32   = 0x1ff; 

pub struct ImixClockManager {}

pub static ImixCM: ImixClockManager = ImixClockManager::new();

impl ImixClockManager {

    const fn new() -> ImixClockManager {
        ImixClockManager {}
    }

    fn convert_to_clock(&self, clock: u32) -> pm::SystemClockSource {
        // Roughly ordered in terms of least to most power consumption
        return match clock {
            RCSYS => pm::SystemClockSource::RcsysAt115kHz,
            RC1M => pm::SystemClockSource::RC1M,
            RCFAST4M => pm::SystemClockSource::RCFAST{
                        frequency: pm::RcfastFrequency::Frequency4MHz},
            RCFAST8M => pm::SystemClockSource::RCFAST{
                        frequency: pm::RcfastFrequency::Frequency8MHz},
            RCFAST12M => pm::SystemClockSource::RCFAST{
                        frequency: pm::RcfastFrequency::Frequency12MHz},
            EXTOSC => pm::SystemClockSource::ExternalOscillator{
                        frequency: pm::OscillatorFrequency::Frequency16MHz,
                        startup_mode: pm::OscillatorStartup::FastStart},
            RC80M => pm::SystemClockSource::RC80M,
            DFLL => pm::SystemClockSource::DfllRc32kAt48MHz,
            _ => pm::SystemClockSource::PllExternalOscillatorAt48MHz{
                        frequency: pm::OscillatorFrequency::Frequency16MHz,
                        startup_mode: pm::OscillatorStartup::FastStart},
        }
    }
}

impl ClockConfigs for ImixClockManager {

    fn get_num_clock_sources(&self) -> u32 {
        9
    }

    fn get_max_freq(&self) -> u32 {
        48_000_000
    }

    fn get_all_clocks(&self) -> u32 {
        ALL_CLOCKS
    }

    fn get_compute(&self) -> u32 {
        PLL 
    }

    fn get_noncompute(&self) -> u32 {
        RCSYS 
    }

    // Used to calculate acceptable clocks based on frequency range
    fn get_clockmask(&self, min_freq: u32, max_freq: u32) -> u32 {
        if min_freq > max_freq {
            return 0;
        }

        let mut clockmask: u32 = 0;

        if min_freq <= 115200 && max_freq >= 115200 { 
            clockmask |= RCSYS;
        } 
        if min_freq <= 1000000 && max_freq >= 1000000 { 
            clockmask |= RC1M;
        }
        if min_freq <= 4300000 && max_freq >= 4300000 { 
            clockmask |= RCFAST4M;
        } 
        if min_freq <= 8200000 && max_freq >= 8200000 { 
            clockmask |= RCFAST8M;
        }
        if min_freq <= 12000000 && max_freq >= 12000000 { 
            clockmask |= RCFAST12M;
        }
        if min_freq <= 16000000 && max_freq >= 16000000 { 
            clockmask |= EXTOSC;
        }
        if min_freq <= 48000000 && max_freq >= 48000000 { 
            clockmask |= DFLL;
            clockmask |= PLL;
        }
        if min_freq <= 40000000 && max_freq >= 40000000 { 
            clockmask |= RC80M;
        }

        return clockmask;
    }


    fn get_clock_frequency(&self, clock: u32) -> u32 {
        let system_clock = self.convert_to_clock(clock);
        pm::get_clock_frequency(system_clock)
    }

    fn get_system_frequency(&self) -> u32 {
        pm::get_system_frequency()
    }

    fn change_system_clock(&self, clock: u32) {
        let system_clock = self.convert_to_clock(clock);
        unsafe {
            pm::PM.change_system_clock(system_clock);
            cortexm4::systick::SysTick::set_hertz(pm::get_system_frequency());
        }
    }

    fn get_intermediates_list(&self, clock:u32) -> IntermediateList {
        let rcfast = RCFAST4M | RCFAST8M | RCFAST12M;
        match clock {
            RCFAST4M | RCFAST8M |RCFAST12M => IntermediateList::new(ALL_CLOCKS & !rcfast, rcfast & !clock),
            _ => IntermediateList::new(0, 0),
        }
    }
}

