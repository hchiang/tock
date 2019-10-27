use core::cell::Cell;
use kernel::common::cells::OptionalCell;
use kernel::hil::clock_pm::*;
use kernel::ReturnCode;
use crate::pm;
use cortexm4;

const NUM_CLOCK_CLIENTS: usize = 10; 
const NUM_CLOCK_SOURCES: usize = 10; //size of SystemClockSource

const RCSYS: u32        = 0x002; 
const RC1M: u32         = 0x004; 
const RCFAST4M: u32     = 0x008; 
const RCFAST8M: u32     = 0x010;    
const RCFAST12M: u32    = 0x020; 
const EXTOSC: u32       = 0x040; 
const RC80M: u32        = 0x080;
const DFLL: u32         = 0x100; 
const PLL: u32          = 0x200; 
const ALLCLOCKS: u32    = 0x3fe;
const DEFAULT: u32      = 0x3ff;
const COMPUTE: u32      = 0x080;

/// Data structure stored by ClockManager for each ClockClient
struct ClockData {
    client: OptionalCell<&'static ClockClient>,
    client_index: Cell<&'static ClientIndex>,
    enabled: Cell<bool>,
    need_lock: Cell<bool>,
    // running is true if a client that does not need a lock has had
    //      client_enabled called
    running: Cell<bool>,
    clockmask: Cell<u32>,
    clocklist: Cell<u32>,
    min_freq: Cell<u32>,
    max_freq: Cell<u32>,
    preferred: Cell<u32>,
}

impl ClockData {
    const fn new(client_index: &'static ClientIndex) -> ClockData {
        ClockData{
            client: OptionalCell::empty(),
            client_index: Cell::new(client_index),
            enabled: Cell::new(false),
            need_lock: Cell::new(true),
            running: Cell::new(false),
            clockmask: Cell::new(ALLCLOCKS),
            clocklist: Cell::new(ALLCLOCKS),
            min_freq: Cell::new(0),
            max_freq: Cell::new(48000000),
            preferred: Cell::new(0),
        }
    }
    fn initialize(&self, client: &'static ClockClient) {
        self.client.set(client);
    }

    fn configure_clock(&self, frequency: u32) {
        let client = self.client.take();
        match client {
            Some(clock_client) => {
                clock_client.configure_clock(frequency);
                self.client.set(clock_client);
            },
            None => {},
        }
    }
    
    fn client_enabled(&self) {
        let client = self.client.take();
        match client {
            Some(clock_client) => {
                clock_client.clock_enabled();
                self.client.set(clock_client);
            },
            None => {},
        }
    }
    fn client_disabled(&self) {
        let client = self.client.take();
        match client {
            Some(clock_client) => {
                clock_client.clock_disabled();
                self.client.set(clock_client);
            },
            None => {},
        }
    }
    fn get_client_index(&self) -> &'static ClientIndex {
        self.client_index.get()
    }
    fn get_enabled(&self) -> bool {
        self.enabled.get()
    }
    fn get_need_lock(&self) -> bool {
        self.need_lock.get()
    }
    fn get_running(&self) -> bool {
        self.running.get()
    }
    fn get_clockmask(&self) -> u32 {
        self.clockmask.get()
    }
    fn get_clocklist(&self) -> u32 {
        self.clocklist.get()
    }
    fn get_min_freq(&self) -> u32 {
        self.min_freq.get()
    }
    fn get_max_freq(&self) -> u32 {
        self.max_freq.get()
    }
    fn get_preferred(&self) -> u32 {
        self.preferred.get()
    }
    fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
    fn set_need_lock(&self, need_lock: bool) {
        self.need_lock.set(need_lock);
    }
    fn set_running(&self, running: bool) {
        self.running.set(running);
    }
    fn set_clockmask(&self, clockmask: u32) {
        self.clockmask.set(clockmask);
    }
    fn set_clocklist(&self, clocklist: u32) {
        self.clocklist.set(clocklist);
    }
    fn set_min_freq(&self, min_freq: u32) {
        self.min_freq.set(min_freq);
    }
    fn set_max_freq(&self, max_freq: u32) {
        self.max_freq.set(max_freq);
    }
    fn set_preferred(&self, preferred: u32) {
        self.preferred.set(preferred);
    }
}

pub struct ImixClockManager {
    clients: [ClockData; NUM_CLOCK_CLIENTS],
    num_clients: Cell<usize>,
    next_client: Cell<usize>,
    current_clock: Cell<u32>,
    change_clock: Cell<bool>,
    lock_count: Cell<u32>,
    // clockmask of clients waiting for a clock change
    change_clockmask: Cell<u32>,
    // clockmask of currently running clients that don't need a lock
    nolock_clockmask: Cell<u32>,
    // number of apps in compute mode
    compute_counter: Cell<u32>,
}

impl ImixClockManager {

    // Used to calculate acceptable clocks based on frequency range
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
        if min_freq <= 48000000 && max_freq >= 48000000 { 
            clockmask |= DFLL;
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
            0x02 => system_clock = pm::SystemClockSource::RcsysAt115kHz,
            0x04 => system_clock = pm::SystemClockSource::RC1M,
            0x08 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency4MHz},
            0x10 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency8MHz},
            0x20 => system_clock = pm::SystemClockSource::RCFAST{
                                    frequency: pm::RcfastFrequency::Frequency12MHz},
            0x40 => system_clock = pm::SystemClockSource::ExternalOscillator{
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            0x80 => system_clock = pm::SystemClockSource::RC80M,
            0x100 => system_clock = pm::SystemClockSource::DfllRc32kAt48MHz,
            0x200 => system_clock = pm::SystemClockSource::PllExternalOscillatorAt48MHz{ 
                                    frequency: pm::OscillatorFrequency::Frequency16MHz,
                                    startup_mode: pm::OscillatorStartup::FastStart},
            _ => system_clock = pm::SystemClockSource::RC80M,
        }
        return system_clock;
    }

    fn update_clock(&self) {
        // Increment lock to prevent recursive calls to update_clock
        self.lock_count.set(self.lock_count.get()+1);
        self.change_clock.set(false);

        // Find a compatible clock
        let mut clockmask = self.nolock_clockmask.get();
        let mut change_clockmask = DEFAULT;
        let mut set_next_client = false;
        let mut next_client = self.next_client.get();
        let mut preferred = 0;
        for _i in 0..self.num_clients.get() { 
            if self.clients[next_client].get_enabled() {
            
                let next_clockmask = clockmask & 
                                    self.clients[next_client].get_clockmask();
                if next_clockmask == 0 {
                    if set_next_client == false {
                        set_next_client = true;
                        self.next_client.set(next_client);
                        self.change_clock.set(true);
                    }
                    let new_change_clockmask = change_clockmask & 
                                        self.clients[next_client].get_clockmask();
                    change_clockmask = new_change_clockmask;
                }
                else {
                    clockmask = next_clockmask;
                    preferred |= self.clients[next_client].get_preferred();
                }
            }
            
            next_client += 1;
            if next_client >= self.num_clients.get() {
                next_client = 0;
            }
        }
        self.change_clockmask.set(change_clockmask);
        if preferred & clockmask != 0 {
            clockmask = preferred;
        }
        if self.compute_counter.get() > 0 && clockmask & COMPUTE != 0 {
            clockmask = 0x1;
        }

        // Choose only one clock from clockmask
        let mut clock = 0x1;
        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1{
                clock = 1 << i;
                break;
            } 
        }

        let clock_changed = self.current_clock.get() != clock;
        self.current_clock.set(clock);

        // Change the clock
        if clock_changed {
            let system_clock = self.convert_to_clock(clock);
            let system_freq = pm::get_clock_frequency(system_clock);
            let old_system_freq = pm::get_system_frequency();
            if old_system_freq < system_freq {
                for i in 0..self.num_clients.get() { 
                    if self.clients[i].get_running() {
                        self.clients[i].configure_clock(system_freq);
                    }
                } 
            }
            unsafe {
                pm::PM.change_system_clock(system_clock);
                cortexm4::systick::SysTick::set_hertz(system_freq);
            }
            if old_system_freq > system_freq {
                for i in 0..self.num_clients.get() { 
                    if self.clients[i].get_running() {
                        self.clients[i].configure_clock(system_freq);
                    }
                } 
            }
        }

        for i in 0..self.num_clients.get() { 
            if !self.clients[i].get_enabled() {
                continue;
            }
            // It's the clock requested by the peripheral
            if clock & self.clients[i].get_clockmask() != 0 {
                if self.clients[i].get_need_lock() {
                    self.lock_count.set(self.lock_count.get()+1);
                    self.clients[i].configure_clock(0);
                    self.clients[i].client_enabled();
                }
                else if !self.clients[i].get_running() {
                    self.clients[i].set_running(true);
                    self.clients[i].configure_clock(0);
                    self.clients[i].client_enabled();
                }
            }
        }
        self.lock_count.set(self.lock_count.get()-1);
    }

    fn update_clockmask(&self, client_index: usize) {
        let freq_clockmask = self.freq_clockmask(
                    self.clients[client_index].get_min_freq(),
                    self.clients[client_index].get_max_freq());
        self.clients[client_index].set_clockmask(
            self.clients[client_index].get_clocklist() & freq_clockmask);
    }
}

impl ChangeClock for ImixClockManager {
    fn change_clock(&self) {
        if self.lock_count.get() == 0 && self.change_clock.get() {
            self.update_clock();
        }
    }

    fn set_compute_mode(&self, compute_mode: bool) {
        let compute_counter = self.compute_counter.get();
        if compute_mode { 
            self.compute_counter.set(compute_counter+1);
            if self.lock_count.get() == 0 && compute_counter == 0 && 
                self.current_clock.get() != 0x1 {
                self.update_clock();
            }
        } else {
            self.compute_counter.set(compute_counter-1);
            if self.lock_count.get() == 0 && compute_counter == 1 &&
                self.current_clock.get() == 0x1 && self.nolock_clockmask.get() != DEFAULT {
                self.update_clock();
            }
        }
    }
}

impl ClockManager for ImixClockManager {
    fn register(&self, client:&'static ClockClient) -> ReturnCode {
        let num_clients = self.num_clients.get();
        if num_clients >= NUM_CLOCK_CLIENTS {
            return ReturnCode::ENOMEM;
        }
        self.clients[num_clients].initialize(client);
        let retval = self.clients[num_clients].get_client_index();
        self.num_clients.set(num_clients+1);
        client.set_client_index(retval);
        return ReturnCode::SUCCESS;
    }
    
    fn enable_clock(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        if self.clients[client_index].get_enabled() {
            self.clients[client_index].client_enabled();
            return Ok(pm::get_system_frequency());
        }

        self.clients[client_index].set_enabled(true);
        let client_clocks = self.clients[client_index].get_clockmask();
        let next_clockmask = self.change_clockmask.get() & client_clocks;

        // The current clock is incompatible
        // This also captures the case where no peripherals are running
        //      if the peripheral's last bit is not set 
        if client_clocks & self.current_clock.get() == 0 {
            self.change_clock.set(true);
            self.change_clockmask.set(next_clockmask);
        }
        // The current clock is compatible and client doesn't need a lock
        else if !self.clients[client_index].get_need_lock() {
            let nolock_clockmask = self.nolock_clockmask.get() & client_clocks;
            // The next clock that will be changed to is also compatible
            if nolock_clockmask & self.change_clockmask.get() != 0 {
                self.nolock_clockmask.set(nolock_clockmask);
                self.clients[client_index].set_running(true);
                self.clients[client_index].client_enabled();
            }
            else {
                self.change_clockmask.set(next_clockmask);
                self.change_clock.set(true);
            }
        }
        // The current clock is compatible and there is no pending clock change
        else if !self.change_clock.get() {
            self.lock_count.set(self.lock_count.get()+1);
            self.clients[client_index].client_enabled();
        }
        else {
             self.change_clockmask.set(next_clockmask);
        }


        return Ok(pm::get_system_frequency());
    }

    fn disable_clock(&self, cidx:&'static ClientIndex) -> ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        if !self.clients[client_index].get_enabled() {
            return ReturnCode::SUCCESS;
        }

        self.clients[client_index].set_enabled(false);
        self.clients[client_index].set_running(false);
        if self.clients[client_index].get_need_lock() {
            self.lock_count.set(self.lock_count.get()-1);
        }
        else {
            // When a lock free client calls disable clock, recalculate 
            // nolock_clockmask
            let num_clients = self.num_clients.get();
            let mut new_clockmask = DEFAULT;
            for i in 0..num_clients { 
                if !self.clients[i].get_need_lock() &&
                        self.clients[i].get_running() {
                    new_clockmask &= self.clients[i].get_clockmask();
                }
            }
            self.nolock_clockmask.set(new_clockmask);
        }
        //TODO this line basically does nothing right now 
        self.clients[client_index].client_disabled();

        if self.lock_count.get() == 0 && self.current_clock.get() != 0x1 {
            self.change_clock.set(true);
        }

        return ReturnCode::SUCCESS;
    }

    // Accessor functions
    fn set_need_lock(&self, cidx:&'static ClientIndex, need_lock: bool) -> ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_need_lock(need_lock);
        return ReturnCode::SUCCESS;
    }
    fn set_clocklist(&self, cidx:&'static ClientIndex, clocklist: u32) -> ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_clocklist(clocklist);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_min_frequency(&self, cidx:&'static ClientIndex, min_freq: u32) ->
                                                        ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_min_freq(min_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_max_frequency(&self, cidx:&'static ClientIndex, max_freq: u32) -> 
                                                        ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_max_freq(max_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }

    fn set_preferred(&self, cidx:&'static ClientIndex, preferred: u32) -> 
                                                        ReturnCode {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_preferred(preferred);
        return ReturnCode::SUCCESS;
    }
    
    fn get_need_lock(&self, cidx:&'static ClientIndex) -> Result<bool, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_need_lock());
    }
    fn get_clocklist(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_clocklist());
    }
    fn get_min_frequency(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_min_freq());
    }
    fn get_max_frequency(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_max_freq());
    }
    fn get_preferred(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_preferred());
    }
}

static IMIX_CLIENT_INDEX0: ClientIndex = ClientIndex::new(0);
static IMIX_CLIENT_INDEX1: ClientIndex = ClientIndex::new(1);
static IMIX_CLIENT_INDEX2: ClientIndex = ClientIndex::new(2);
static IMIX_CLIENT_INDEX3: ClientIndex = ClientIndex::new(3);
static IMIX_CLIENT_INDEX4: ClientIndex = ClientIndex::new(4);
static IMIX_CLIENT_INDEX5: ClientIndex = ClientIndex::new(5);
static IMIX_CLIENT_INDEX6: ClientIndex = ClientIndex::new(6);
static IMIX_CLIENT_INDEX7: ClientIndex = ClientIndex::new(7);
static IMIX_CLIENT_INDEX8: ClientIndex = ClientIndex::new(8);
static IMIX_CLIENT_INDEX9: ClientIndex = ClientIndex::new(9);

pub static mut CM: ImixClockManager = ImixClockManager {
    
    clients: [ClockData::new(&IMIX_CLIENT_INDEX0), 
              ClockData::new(&IMIX_CLIENT_INDEX1),
              ClockData::new(&IMIX_CLIENT_INDEX2),
              ClockData::new(&IMIX_CLIENT_INDEX3),
              ClockData::new(&IMIX_CLIENT_INDEX4),
              ClockData::new(&IMIX_CLIENT_INDEX5),
              ClockData::new(&IMIX_CLIENT_INDEX6),
              ClockData::new(&IMIX_CLIENT_INDEX7),
              ClockData::new(&IMIX_CLIENT_INDEX8),
              ClockData::new(&IMIX_CLIENT_INDEX9)],
    num_clients: Cell::new(0),
    next_client: Cell::new(0),
    current_clock: Cell::new(0),
    change_clock: Cell::new(false),
    lock_count: Cell::new(0),
    change_clockmask: Cell::new(DEFAULT),
    nolock_clockmask: Cell::new(DEFAULT),
    compute_counter: Cell::new(0),
};

