use core::cell::Cell;
use kernel::hil::clock_pm::*;
use kernel::ReturnCode;
use pm;

const NUM_CLOCK_CLIENTS: usize = 10; 
const NUM_CLOCK_SOURCES: usize = 10; //size of SystemClockSource

const RC1M: u32         = 0x002; 
const RCFAST4M: u32     = 0x004; 
const RCFAST8M: u32     = 0x008;    
const RCFAST12M: u32    = 0x010; 
const EXTOSC: u32       = 0x020; 
const DFLL: u32         = 0x040; 
const PLL: u32          = 0x080; 
const RC80M: u32        = 0x100; 
const RCSYS: u32        = 0x200; 

pub struct ImixClockManager<'a> {
    clients: [ClockData<'a>; NUM_CLOCK_CLIENTS],
    num_clients: Cell<usize>,
    next_client: Cell<usize>,
    current_clock: Cell<u32>,
    change_clock: Cell<bool>,
    lock_count: Cell<u32>,
}

impl ImixClockManager<'a> {

    fn freq_clockmask(&self, min_freq: u32, max_freq: u32) -> u32 {
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
        if min_freq <= 48000000 && max_freq >= 20000000 { 
            clockmask |= DFLL;
        }
        if min_freq <= 48000000 && max_freq >= 48000000 { 
            clockmask |= PLL;
        }
        if min_freq <= 40000000 && max_freq >= 40000000 { 
            clockmask |= RC80M;
        }

        return clockmask;
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

    fn update_clock(&self) {
        let mut clockmask: u32 = 0xffffffff;
        self.change_clock.set(false);

        //TODO first check clients that don't hold lock
        for _i in 0..self.num_clients.get() { 
            let next_client = self.next_client.get();
            if !self.clients[next_client].get_enabled() {
                continue;
            }
            let next_clockmask = clockmask & self.clients[next_client].get_clocklist();
            if next_clockmask == 0 { 
                self.change_clock.set(true);
                break;
            }
            clockmask = next_clockmask;
            
            self.next_client.set(next_client+1);
            if self.next_client.get() >= self.num_clients.get() {
                self.next_client.set(0);
            }
        }

        let mut clock = 0x1;
        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1{
                clock = 1 << i;
                break;
            } 
        }
        let clock_changed = self.current_clock.get() != clock;
        self.current_clock.set(clock);

        if clock_changed {
            let system_clock = self.convert_to_clock(clock);
            unsafe {
                pm::PM.change_system_clock(system_clock);
            }
        }

        self.lock_count.set(self.lock_count.get()+1);
        for i in 0..self.num_clients.get() { 
            if self.clients[i].get_enabled() && (clock & self.clients[i].get_clocklist() != 0) {
                self.lock_count.set(self.lock_count.get()+1);
                self.clients[i].client_update();
            }
        }
        self.lock_count.set(self.lock_count.get()-1);
        if self.lock_count.get() == 0 && self.change_clock.get() {
            self.update_clock();
        }
    }

    fn update_clockmask(&self, client_index: usize) {
        self.clients[client_index].set_clockmask(
            self.clients[client_index].get_clocklist() &
            self.freq_clockmask(
                self.clients[client_index].get_min_freq(),
                self.clients[client_index].get_max_freq()));
    }
}

impl<'a> ClockManager<'a> for ImixClockManager<'a> {

    fn register(&self, c:&'a ClockClient) {
        let num_clients = self.num_clients.get();
        self.clients[num_clients].initialize(c);
        c.enable_cm(num_clients);
        self.num_clients.set(num_clients+1);
    }
    
    fn enable_clock(&self, client_index: usize) -> ReturnCode {
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        if self.clients[client_index].get_enabled() {
            return ReturnCode::SUCCESS;
        }

        self.clients[client_index].set_enabled(true);
        if self.clients[client_index].get_clockmask() & self.current_clock.get() == 0 {
            self.change_clock.set(true);
            if self.lock_count.get() == 0 {
                self.update_clock();
            }
        }
        else if !self.change_clock.get() {
            self.lock_count.set(self.lock_count.get()+1);
            self.clients[client_index].client_update();
        }
        return ReturnCode::SUCCESS;
    }

    //Automatically calls update_clock if there are no locks 
    fn disable_clock(&self, client_index: usize) -> ReturnCode {
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        if !self.clients[client_index].get_enabled() {
            return ReturnCode::SUCCESS;
        }

        self.clients[client_index].set_enabled(false);
        self.lock_count.set(self.lock_count.get()-1);
        if self.lock_count.get() == 0 {
            self.update_clock();
        }
        return ReturnCode::SUCCESS;
    }

    fn set_need_lock(&self, client_index: usize, need_lock: bool) -> ReturnCode {
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_need_lock(need_lock);
        return ReturnCode::SUCCESS;
    }

    fn set_clocklist(&self, client_index: usize, clocklist: u32) -> ReturnCode {
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_clocklist(clocklist);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_min_frequency(&self, client_index: usize, min_freq: u32) -> ReturnCode {
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_min_freq(min_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_max_frequency(&self, client_index: usize, max_freq: u32) -> ReturnCode{
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_max_freq(max_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    
    fn get_need_lock(&self, client_index: usize) -> Result<bool, ReturnCode> {
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_need_lock());
    }
    fn get_clocklist(&self, client_index: usize) -> Result<u32, ReturnCode> {
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_clocklist());
    }
    fn get_min_frequency(&self, client_index: usize) -> Result<u32, ReturnCode> {
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_min_freq());
    }
    fn get_max_frequency(&self, client_index: usize) -> Result<u32, ReturnCode> {
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_max_freq());
    }
}
pub static mut CM: ImixClockManager = ImixClockManager {
    
    clients: [ClockData::new(), ClockData::new(), ClockData::new(), 
                ClockData::new(), ClockData::new(), ClockData::new(), 
                ClockData::new(), ClockData::new(), ClockData::new(), 
                ClockData::new()],
    num_clients: Cell::new(0),
    next_client: Cell::new(0),
    current_clock: Cell::new(0),
    change_clock: Cell::new(false),
    lock_count: Cell::new(0),
};

