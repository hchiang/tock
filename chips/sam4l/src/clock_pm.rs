use kernel::hil::clock_pm::{ClockManager,ClockClient,ClockParams};
use pm;
use core::cmp;
use kernel::common::{List};

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
                    let next_min_freq = cmp::max(min_freq,
                        param.min_frequency.get());
                    let next_max_freq = cmp::min(max_freq,
                        param.max_frequency.get());
                    let next_clockmask = clockmask & param.clocklist.get() &
                        self.freq_clockmask(next_min_freq, next_max_freq);
                    if next_clockmask == 0 { 
                        for _i in 0..client_ctr {
                            let client_node = self.clients.pop_head();
                            match client_node {
                                Some(add_node) => { 
                                    self.clients.push_tail(add_node);
                                }
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

    fn freq_clockmask(&self, min_freq: u32, max_freq: u32) -> u32 {
        if min_freq > max_freq {
            return 0;
        }

        let min_clockmask: u32;
        let max_clockmask: u32;

        if min_freq <= 115200 { 
            min_clockmask = 0x3fe;
        } else if min_freq <= 1000000 { 
            min_clockmask = 0x1fe;
        } else if min_freq <= 4300000 { 
            min_clockmask = 0x1fc;
        } else if min_freq <= 8200000 { 
            min_clockmask = 0x1f8;
        } else if min_freq <= 12000000 { 
            min_clockmask = 0x1f0;
        } else if min_freq <= 1600000 { 
            min_clockmask = 0x1e0;
        } else if min_freq <= 40000000 { 
            min_clockmask = 0x1c0;
        } else {
            min_clockmask = 0x0c0;
        }

        if max_freq >= 48000000 {
            max_clockmask = 0x3fe;
        } else if max_freq >= 40000000 {
            max_clockmask = 0x37e;
        } else if max_freq >= 20000000 {
            max_clockmask = 0x27e;
        } else if max_freq >= 16000000 {
            max_clockmask = 0x23e;
        } else if max_freq >= 12000000 {
            max_clockmask = 0x21e;
        } else if max_freq >= 8200000 {
            max_clockmask = 0x20e;
        } else if max_freq >= 4300000 {
            max_clockmask = 0x206;
        } else if max_freq >= 1000000 {
            max_clockmask = 0x202;
        } else {
            max_clockmask = 0x200;
        }

        return min_clockmask & max_clockmask;
    
    }

    fn convert_to_clock(&self, clock: u32) -> pm::SystemClockSource {
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

    fn update_clock(&mut self) {

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
        //TODO frequency check
        if !self.change_clock {
            if (params.clocklist.get() >> self.current_clock) & 0b1 == 0 {
                return true;
            }
        }
        return false;
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

