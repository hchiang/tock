use kernel::hil::clock_pm::*;
use pm;

const NUM_CLOCK_CLIENTS: usize = 10; 
const NUM_CLOCK_SOURCES: usize = 10; //size of SystemClockSource

pub struct ImixClockManager<'a> {
    clients: [ClockData<'a>; NUM_CLOCK_CLIENTS],
    num_clients: usize,
    next_client: usize,
    current_clock: u32,
    change_clock: bool,
    lock_count: u32,
}

impl ImixClockManager<'a> {

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
        // Roughly ordered in terms of least to most power consumption
        let mut system_clock = pm::SystemClockSource::RcsysAt115kHz;
        match clock {
            0x02 => system_clock = pm::SystemClockSource::RC1M,
            0x04 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency4MHz},
            0x08 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency8MHz},
            0x10 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency12MHz},
            0x20 => system_clock = pm::SystemClockSource::ExternalOscillator{
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            0x40 => system_clock = pm::SystemClockSource::DfllRc32kAt48MHz,
            0x80 => system_clock = pm::SystemClockSource::PllExternalOscillatorAt48MHz{ 
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            0x100 => system_clock = pm::SystemClockSource::RC80M,
            0x200 => system_clock = pm::SystemClockSource::RcsysAt115kHz,
            _ => system_clock = pm::SystemClockSource::DfllRc32kAt48MHz,
        }
        return system_clock;
    }

    fn update_clock(&mut self) {
        let mut clockmask: u32 = 0xffffffff;
        self.change_clock = false;

        for _i in 0..self.num_clients { 
            if !self.clients[self.next_client].enabled {
                continue;
            }
            let next_clockmask = clockmask & self.clients[self.next_client].clock_mask;
            if next_clockmask == 0 { 
                self.change_clock = true;
                break;
            }
            clockmask = next_clockmask;
            
            self.next_client += 1;
            if self.next_client >= self.num_clients {
                self.next_client = 0;
            }
        }

        let mut clock = 0x1;
        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1{
                clock = 1 << i;
            } 
        }
        let clock_changed = self.current_clock != clock;
        self.current_clock = clock;

        if clock_changed {
            let system_clock = self.convert_to_clock(clock);
            unsafe {
                pm::PM.change_system_clock(system_clock);
            }
        }

        self.lock_count += 1;
        for i in 0..self.num_clients { 
            if self.clients[i].enabled && (clock & self.clients[i].clock_mask != 0) {
                self.lock_count += 1;
                match self.clients[i].client {
                    Some(clock_client) => {clock_client.clock_updated();},
                    None => {},
                }
            }
        }
        self.lock_count -= 1;
        if self.lock_count == 0 && self.change_clock {
            self.update_clock();
        }
    }
}

impl<'a> ClockManager<'a> for ImixClockManager<'a> {

    fn register(&mut self, c:&'a ClockClient) {
        self.clients[self.num_clients].client = Some(c);
        c.enable_cm(self.num_clients);
        self.num_clients += 1;
    }
    
    //Automatically calls clock_change if possible after a peripheral calls unlock
    fn unlock(&mut self, client_index: usize) {
        self.clients[client_index].enabled = false;
        self.lock_count -= 1;
        if self.lock_count == 0 {
            self.update_clock();
        }
    }

    fn clock_change(&mut self, client_index: usize, params: &ClockParams) {
        if (params.min_frequency.get() != self.clients[client_index].min_freq) ||
            (params.max_frequency.get() != self.clients[client_index].max_freq) {
            self.clients[client_index].min_freq = params.min_frequency.get();
            self.clients[client_index].max_freq = params.max_frequency.get();
            self.clients[client_index].clock_mask = params.clocklist.get() &
                self.freq_clockmask(params.min_frequency.get(),
                    params.max_frequency.get());
        }

        self.clients[client_index].enabled = true;
        if self.clients[client_index].clock_mask & self.current_clock == 0 {
            self.change_clock=true;
            if self.lock_count == 0 {
                self.update_clock();
            }
        }
        else if !self.change_clock {
            self.lock_count += 1;
            match self.clients[client_index].client {
                Some(clock_client) => {clock_client.clock_updated();},
                None => {},
            }
        }
    }
}
pub static mut CM: ImixClockManager = ImixClockManager {
    clients: [ ClockData{
        client: None,
        enabled: false,
        clock_mask: 0,
        min_freq: 0,
        max_freq: 0, } ; NUM_CLOCK_CLIENTS],
    num_clients: 0,
    next_client: 0,
    current_clock: 99,
    change_clock: false,
    lock_count: 0,
};

