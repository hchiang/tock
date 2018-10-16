use kernel::hil::clock_pm::{ClockManager,ClockClient,ClockParams};
use pm;
use core::cmp;
use kernel::common::{List,ListLink};

const NUM_CLOCK_SOURCES: usize = 10; //size of SystemClockSource

pub struct ImixClockManager<'a> {
    clients: List<'a, ClockClient<'a> + 'a>,
    current_clock: u32,
    change_clock: bool,
    lock_count: u32,
}

impl<'a> ImixClockManager<'a> {

    fn choose_clock(&mut self) -> u32 {
        let mut clockmask : u32 = 0xffffffff;
        let mut min_freq : u32 = 0;
        let mut max_freq : u32 = 48000000;
        let mut client_ctr = 0;

        for client in self.clients.iter() { 
            let param = client.get_params();
            match param {
                Some(param) => {
                    let next_clockmask = clockmask & param.clocklist.get();
                    let next_min_freq = cmp::max(min_freq,
                        param.min_frequency.get());
                    let next_max_freq = cmp::min(max_freq,
                        param.max_frequency.get());
                    if next_clockmask == 0 || (next_min_freq >= next_max_freq) {
                        for i in 0..client_ctr {
                            let client_node = self.clients.pop_head();
                            match client_node {
                                Some(add_node) => { self.clients.push_tail(add_node); }
                                None => {}
                            }
                        } 
                        for i in 0..NUM_CLOCK_SOURCES {
                            if (clockmask >> i) & 0b1 == 1{
                                return i as u32;
                            } 
                        }
                    }
                    clockmask = next_clockmask;
                    min_freq = next_min_freq;
                    max_freq = next_max_freq;
                },
                None => continue,
            }
            client_ctr += 1;
        }

        self.change_clock = false;
        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1{
                return i as u32;
            } 
        }
        return 0;
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

    fn update_clock(&mut self){

        let clock = self.choose_clock();
        let clock_changed = self.current_clock != clock;
        self.current_clock = clock;

        if clock_changed {
            let system_clock = self.convert_to_clock(clock);
            unsafe {
                pm::PM.change_system_clock(system_clock);
            }
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
        //self.client_ptr = Some(self.clients).head();
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
        if self.lock_count == 0 {
            self.lock_count += 1;
            self.update_clock();
            self.lock_count -= 1;
        }
    }

    fn need_clock_change(&self,params:&ClockParams)->bool{
        if !self.change_clock {
            if (params.clocklist.get() >> self.current_clock) & 0b1 == 1 {
                return false;
            }
        }
        return true;
    }

    fn clock_change(&mut self){
        self.change_clock=true;
        if self.lock_count == 0 {
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

