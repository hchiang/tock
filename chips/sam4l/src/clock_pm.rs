use kernel::hil::clock_pm::{ClockManager,ClockClient};
//use core::cell::Cell;
use core::u32::MAX;
use core::u32::MIN;
use core::cmp;
use pm;
use kernel::common::List;
#[macro_use(debug_gpio)]

const NUM_CLOCK_SOURCES: usize = 9; //size of SystemClockSource

//TODO: option to turn off ClockManager for time important peripherals 

pub struct ImixClockManager<'a> {
    clients: List<'a, ClockClient<'a> + 'a>,
    current_clock: u32,
    change_clock: bool,
    lock_count: u32,
}

impl<'a> ImixClockManager<'a> {

    fn choose_clock(&self) -> u32 {
        //Assume there will always be a clock that works for all peripherals
        //TODO: also choose frequency
        //TODO: bus assignments + clock prescaling per bus depending on the peripherals 

        let mut clockmask : u32 = 0xffffffff;
        let mut max_freq : u32 = MAX;
        let mut min_freq : u32 = MIN;

        for client in self.clients.iter() {
            let param = client.get_params();
            match param {
                Some(param) => {
                    clockmask &= param.clocklist.get();
                    max_freq = cmp::min(max_freq,param.max_frequency.get());
                    min_freq = cmp::max(min_freq,param.min_frequency.get());           
                },
                None => continue,
            }
        }

        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1 {
                return i as u32;
            } 
        }
        return 0;
    }

    pub fn update_clock(&mut self){

        let clock = self.choose_clock();

        let mut system_clock = pm::SystemClockSource::RcsysAt115kHz;
        //Roughly ordered in terms of least to most power consumption
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
            _ => system_clock = pm::SystemClockSource::RcsysAt115kHz,
        }

        let clock_changed = self.current_clock != clock;
        self.current_clock = clock;

        if clock_changed {
            debug_gpio!(0,set);
            unsafe {
                pm::PM.change_system_clock(system_clock);
            }
            debug_gpio!(0,clear);
        }

        for client in self.clients.iter() {
            client.clock_updated(clock_changed);
        }
    }
}

impl<'a> ClockManager<'a> for ImixClockManager<'a> {

    fn register(&mut self, c:&'a ClockClient<'a>) {
        self.clients.push_head(c);
        c.enable_cm();
    }
    
    fn lock(&mut self) -> bool {
        if !self.change_clock{
            self.lock_count += 1;
            return true;
        }
        return false;
    }
    
    //Automatically calls clock_change if possible after a peripheral calls unlock
    fn unlock(&mut self){
        self.lock_count -= 1;
        //if (self.lock_count == 0) & self.change_clock {
        if self.lock_count == 0 {
            self.clock_change();
        }
    }

    fn clock_change(&mut self){
        self.change_clock=true;
        if self.lock_count == 0 {
            self.change_clock = false;
            //locking prevents interrupts from causing nested clock_change
            self.lock_count += 1;
            self.update_clock();
            self.lock_count -= 1;
        }
    }
}
pub static mut CM: ImixClockManager = ImixClockManager {
    clients: List::new(),
    current_clock: 9,
    change_clock: false,
    lock_count: 0,
};

