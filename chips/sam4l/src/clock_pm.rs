use core::cell::Cell;
use kernel::common::cells::OptionalCell;
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

#[derive(Copy, Clone)]
struct ImixClientIndex {
    client_index: usize,
}

impl ImixClientIndex {
    const fn new(client_index: usize) -> ImixClientIndex {
        ImixClientIndex {
            client_index: client_index,
        }
    }
}

impl ClientIndex for ImixClientIndex {
    fn get(&self) -> usize {
        self.client_index
    }
}

/// Data structure stored by ClockManager for each ClockClient
pub struct ClockData<'a> {
    client: OptionalCell<&'a ClockClient>,
    client_index: Cell<&'static ImixClientIndex>,
    enabled: Cell<bool>,
    need_lock: Cell<bool>,
    // running is used to note whether a client that does not require a lock has
    // started operation
    running: Cell<bool>,
    clockmask: Cell<u32>,
    clocklist: Cell<u32>,
    min_freq: Cell<u32>,
    max_freq: Cell<u32>,
}

impl ClockData<'a>{
    const fn new(client_index: &'static ImixClientIndex) -> ClockData<'a> {
        ClockData{
            client: OptionalCell::empty(),
            client_index: Cell::new(client_index),
            enabled: Cell::new(false),
            need_lock: Cell::new(true),
            running: Cell::new(false),
            clockmask: Cell::new(0x3ff),
            clocklist: Cell::new(0x3ff),
            min_freq: Cell::new(0),
            max_freq: Cell::new(48000000),
        }
    }
    fn initialize(&self, client: &'a ClockClient) {
        self.client.set(client);
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
}

pub struct ImixClockManager<'a> {
    clients: [ClockData<'a>; NUM_CLOCK_CLIENTS],
    num_clients: Cell<usize>,
    next_client: Cell<usize>,
    current_clock: Cell<u32>,
    change_clock: Cell<bool>,
    lock_count: Cell<u32>,
    // clockmask of clients waiting for a clock change
    change_clockmask: Cell<u32>,
    // clockmask of currently running clients that don't need a lock
    nolock_clockmask: Cell<u32>,
}

impl ImixClockManager<'a> {

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
        self.change_clock.set(false);

        // Find a compatible clock
        let mut clockmask = self.nolock_clockmask.get();
        let mut change_clockmask = 0xfff;
        let mut set_next_client = false;
        let mut next_client = self.next_client.get();
        for _i in 0..self.num_clients.get() { 
            if !self.clients[next_client].get_enabled() {
                continue;
            }
            let next_clockmask = clockmask & 
                                self.clients[next_client].get_clocklist();
            if next_clockmask == 0 {
                if set_next_client == false {
                    set_next_client = true;
                    self.next_client.set(next_client);
                    self.change_clock.set(true);
                }
                let new_change_clockmask = change_clockmask & 
                                    self.clients[next_client].get_clocklist();
                if new_change_clockmask != 0 {
                    change_clockmask = new_change_clockmask;
                }
            }
            else {
                clockmask = next_clockmask;
            }
            
            next_client += 1;
            if next_client >= self.num_clients.get() {
                next_client = 0;
            }
        }
        self.change_clockmask.set(change_clockmask);
        // Choose only one clock from clockmask
        let mut clock = 0x1;
        for i in 0..NUM_CLOCK_SOURCES {
            if (clockmask >> i) & 0b1 == 1{
                clock = 1 << i;
                break;
            } 
        }

        // Change the clock
        let clock_changed = self.current_clock.get() != clock;
        self.current_clock.set(clock);
        if clock_changed {
            let system_clock = self.convert_to_clock(clock);
            unsafe {
                pm::PM.change_system_clock(system_clock);
            }
        }

        // Increment lock to prevent recursive calls to update_clock
        self.lock_count.set(self.lock_count.get()+1);
        for i in 0..self.num_clients.get() { 
            if !self.clients[i].get_enabled() {
                continue;
            }
            if clock & self.clients[i].get_clocklist() != 0 {
                if self.clients[i].get_need_lock() {
                    self.lock_count.set(self.lock_count.get()+1);
                    self.clients[i].client_enabled();
                }
                else if !self.clients[i].get_running() {
                    self.clients[i].set_running(true);
                    self.clients[i].client_enabled();
                }
            }
        }
        self.lock_count.set(self.lock_count.get()-1);

        // In case the clock chosen is not compatible with all clocks, need 
        // another clock change
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
        if num_clients >= NUM_CLOCK_CLIENTS {
            //return Err(ReturnCode::ENOMEM);
            return;
        }
        self.clients[num_clients].initialize(c);
        self.num_clients.set(num_clients+1);
        c.enable_cm(self.clients[num_clients].get_client_index());
    }
    
    fn enable_clock(&self, cidx:&'static ClientIndex) -> ReturnCode {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        if self.clients[client_index].get_enabled() {
            return ReturnCode::SUCCESS;
        }

        self.clients[client_index].set_enabled(true);

        // The current clock is incompatible
        if self.clients[client_index].get_clockmask() & 
                            self.current_clock.get() == 0 {
            self.change_clock.set(true);
            // Choose what the next clock will be
            let next_clockmask = self.change_clockmask.get() & 
                                self.clients[client_index].get_clocklist();
            if next_clockmask != 0 { 
                self.change_clockmask.set(next_clockmask);
            }

            if self.lock_count.get() == 0 {
                self.update_clock();
            }
        }
        // The current clock is compatible and client doesn't need a lock
        else if !self.clients[client_index].get_need_lock() {
            let nolock_clockmask = self.nolock_clockmask.get() & 
                                self.clients[client_index].get_clocklist();

            // The next clock that will be changed to is also compatible
            if nolock_clockmask & self.change_clockmask.get() != 0 {
                self.nolock_clockmask.set(nolock_clockmask);
                self.clients[client_index].set_running(true);
                self.clients[client_index].client_enabled();
            }
        }
        // The current clock is compatible and there is no pending clock change
        else if !self.change_clock.get() {
            self.lock_count.set(self.lock_count.get()+1);
            self.clients[client_index].client_enabled();
        }
        return ReturnCode::SUCCESS;
    }

    fn disable_clock(&self, cidx:&'static ClientIndex) -> ReturnCode {
        let client_index = cidx.get();
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
            self.clients[client_index].client_disabled();
            // Automatically calls update_clock if there are no locks
            if self.lock_count.get() == 0 {
                self.update_clock();
            }
        }
        else {
            // When a lock free client calls disable clock, recalculate 
            // nolock_clockmask
            let num_clients = self.num_clients.get();
            let mut new_clockmask = 0xfff;
            for i in 0..num_clients { 
                if !self.clients[i].get_need_lock() &&
                        self.clients[i].get_running() {
                    new_clockmask &= self.clients[i].get_clocklist();
                }
            }
            self.nolock_clockmask.set(new_clockmask);
            self.clients[client_index].client_disabled();
        }
        return ReturnCode::SUCCESS;
    }

    // Accessor functions
    fn set_need_lock(&self, cidx:&'static ClientIndex, need_lock: bool) -> ReturnCode {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_need_lock(need_lock);
        return ReturnCode::SUCCESS;
    }
    fn set_clocklist(&self, cidx:&'static ClientIndex, clocklist: u32) -> ReturnCode {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_clocklist(clocklist);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_min_frequency(&self, cidx:&'static ClientIndex, min_freq: u32) ->
                                                        ReturnCode {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_min_freq(min_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    fn set_max_frequency(&self, cidx:&'static ClientIndex, max_freq: u32) -> 
                                                        ReturnCode {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return ReturnCode::EINVAL;
        }
        self.clients[client_index].set_max_freq(max_freq);
        self.update_clockmask(client_index);
        return ReturnCode::SUCCESS;
    }
    
    fn get_need_lock(&self, cidx:&'static ClientIndex) -> Result<bool, ReturnCode> {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_need_lock());
    }
    fn get_clocklist(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_clocklist());
    }
    fn get_min_frequency(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_min_freq());
    }
    fn get_max_frequency(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }
        return Ok(self.clients[client_index].get_max_freq());
    }
}

static IMIX_CLIENT_INDEX0: ImixClientIndex = ImixClientIndex::new(0);
static IMIX_CLIENT_INDEX1: ImixClientIndex = ImixClientIndex::new(1);
static IMIX_CLIENT_INDEX2: ImixClientIndex = ImixClientIndex::new(2);
static IMIX_CLIENT_INDEX3: ImixClientIndex = ImixClientIndex::new(3);
static IMIX_CLIENT_INDEX4: ImixClientIndex = ImixClientIndex::new(4);
static IMIX_CLIENT_INDEX5: ImixClientIndex = ImixClientIndex::new(5);
static IMIX_CLIENT_INDEX6: ImixClientIndex = ImixClientIndex::new(6);
static IMIX_CLIENT_INDEX7: ImixClientIndex = ImixClientIndex::new(7);
static IMIX_CLIENT_INDEX8: ImixClientIndex = ImixClientIndex::new(8);
static IMIX_CLIENT_INDEX9: ImixClientIndex = ImixClientIndex::new(9);

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
    change_clockmask: Cell::new(0xfff),
    nolock_clockmask: Cell::new(0xfff),
};

