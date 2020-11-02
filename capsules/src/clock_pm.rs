use core::cell::Cell;
use kernel::common::cells::OptionalCell;
use kernel::hil::clock_pm::*;
use kernel::ReturnCode;
use kernel::debug_gpio;

pub const NUM_CLOCK_CLIENTS: usize = 10; 

pub static CLIENT_INDEX0: ClientIndex = ClientIndex::new(0);
pub static CLIENT_INDEX1: ClientIndex = ClientIndex::new(1);
pub static CLIENT_INDEX2: ClientIndex = ClientIndex::new(2);
pub static CLIENT_INDEX3: ClientIndex = ClientIndex::new(3);
pub static CLIENT_INDEX4: ClientIndex = ClientIndex::new(4);
pub static CLIENT_INDEX5: ClientIndex = ClientIndex::new(5);
pub static CLIENT_INDEX6: ClientIndex = ClientIndex::new(6);
pub static CLIENT_INDEX7: ClientIndex = ClientIndex::new(7);
pub static CLIENT_INDEX8: ClientIndex = ClientIndex::new(8);
pub static CLIENT_INDEX9: ClientIndex = ClientIndex::new(9);

/// Data structure stored by ClockManager for each ClockClient
pub struct ClockData {
    client: OptionalCell<&'static dyn ClockClient>,
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
}

impl ClockData {
    pub fn new(client_index: &'static ClientIndex, all_clocks: u32, max_freq: u32) -> ClockData {
        ClockData{
            client: OptionalCell::empty(),
            client_index: Cell::new(client_index),
            enabled: Cell::new(false),
            need_lock: Cell::new(true),
            running: Cell::new(false),
            clockmask: Cell::new(all_clocks),
            clocklist: Cell::new(all_clocks),
            min_freq: Cell::new(0),
            max_freq: Cell::new(max_freq),
        }
    }
    fn initialize(&self, client: &'static dyn ClockClient) {
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
    fn get_client_index(&self) -> &'a ClientIndex {
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

pub struct ClockManagement<'a> {
    configs: &'a dyn ClockConfigs,
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
    compute_mode: Cell<bool>,
}

impl ClockManagement<'a> {

    pub fn new(configs: &'a dyn ClockConfigs,
                clients: [ClockData; NUM_CLOCK_CLIENTS])
                -> ClockManagement<'a> {
        ClockManagement {
            configs: configs,
            clients: clients, 
            num_clients: Cell::new(0),
            next_client: Cell::new(0),
            current_clock: Cell::new(0),
            change_clock: Cell::new(false),
            lock_count: Cell::new(0),
            change_clockmask: Cell::new(0xffffffff),
            nolock_clockmask: Cell::new(0xffffffff),
            compute_counter: Cell::new(0),
            compute_mode: Cell::new(false),
        }
    }

    fn update_clock(&self) {
        // Increment lock to prevent recursive calls to update_clock
        self.lock_count.set(self.lock_count.get()+1);
        self.change_clock.set(false);

        // Find a clock compatible with running peripherals
        let mut clockmask = self.nolock_clockmask.get();

        // Remove options that need to go through incompatible intermediates
        let intermediates = self.configs.get_intermediates_list(self.current_clock.get());
        if intermediates.get_intermediates() != 0 {
            if clockmask & intermediates.get_intermediates()== 0 {
                clockmask = clockmask & !intermediates.get_ends();
                if clockmask == 0 { return; }
            }
        }

        let mut change_clockmask = self.configs.get_all_clocks();
        let mut set_next_client = false;
        let mut next_client = self.next_client.get();
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
                }
            }
            
            next_client += 1;
            if next_client >= self.num_clients.get() {
                next_client = 0;
            }
        }
        self.change_clockmask.set(change_clockmask);

        let mut clock = self.configs.get_compute();
        // if there are no peripherals running OR
        // if compute mode requested AND the compute clock is compatible AND
        // a low power inefficient clock is likely to be chosen
        if clockmask > self.configs.get_all_clocks() ||
            self.compute_counter.get() > 0 && clockmask & self.configs.get_compute() != 0 &&
            clockmask & self.configs.get_noncompute() != 0 {
            self.compute_mode.set(true);
        }
        else {
            self.compute_mode.set(false);
            // Choose only one clock from clockmask
            for i in 0..self.configs.get_num_clock_sources() {
                if (clockmask >> i) & 0b1 == 1{
                    clock = 1 << i;
                    break;
                } 
            }
        }

        let clock_changed = self.current_clock.get() != clock;

        // Change the clock
        let mut system_freq = 0;
        if clock_changed {
            system_freq = self.configs.get_clock_frequency(clock);
            let current_freq = self.configs.get_system_frequency();
            if current_freq < system_freq {
                for i in 0..self.num_clients.get() { 
                    if self.clients[i].get_running() {
                        self.clients[i].configure_clock(system_freq);
                    }
                } 
            }

            //self.configs.change_system_clock(clock);
            if current_freq > system_freq {
                for i in 0..self.num_clients.get() { 
                    if self.clients[i].get_running() {
                        self.clients[i].configure_clock(system_freq);
                    }
                } 
            }
        }

        self.current_clock.set(clock);
        for i in 0..self.num_clients.get() { 
            if !self.clients[i].get_enabled() {
                continue;
            }
            // It's the clock requested by the peripheral
            if clock & self.clients[i].get_clockmask() != 0 {
                if self.clients[i].get_need_lock() {
                    self.lock_count.set(self.lock_count.get()+1);
                    self.clients[i].configure_clock(system_freq);
                    self.clients[i].client_enabled();
                }
                else if !self.clients[i].get_running() {
                    self.clients[i].set_running(true);
                    self.clients[i].configure_clock(system_freq);
                    self.clients[i].client_enabled();
                }
            }
        }
        self.lock_count.set(self.lock_count.get()-1);
    }

    fn update_clockmask(&self, client_index: usize) {
        let freq_clockmask = self.configs.get_clockmask(
                    self.clients[client_index].get_min_freq(),
                    self.clients[client_index].get_max_freq());
        self.clients[client_index].set_clockmask(
            self.clients[client_index].get_clocklist() & freq_clockmask);
    }
}

impl ChangeClock for ClockManagement<'a> {
    fn change_clock(&self) {
        if self.lock_count.get() == 0 && self.change_clock.get() {
            self.update_clock();
        }
    }

    fn set_compute_mode(&self, compute_mode: bool) {
        let compute_counter = self.compute_counter.get();
        let current_clock = self.current_clock.get();
        if compute_mode { 
            self.compute_counter.set(compute_counter+1);

            if self.lock_count.get() == 0 && compute_counter == 0 && 
                (current_clock & self.configs.get_noncompute() != 0 || !self.compute_mode.get() && 
                self.nolock_clockmask.get() > self.configs.get_all_clocks()) {
                self.update_clock();
            }
        } else {
            self.compute_counter.set(compute_counter-1);
            if self.lock_count.get() == 0 && compute_counter == 1 &&
                self.compute_mode.get() && self.nolock_clockmask.get() <= self.configs.get_all_clocks() {
                self.change_clock.set(true);
            }
        }
    }
}

impl ClockManager for ClockManagement<'a> {
    fn register(&'static self, client:&'static dyn ClockClient) -> ReturnCode {
        let num_clients = self.num_clients.get();
        if num_clients >= NUM_CLOCK_CLIENTS {
            return ReturnCode::ENOMEM;
        }
        self.clients[num_clients].initialize(client);
        let retval = self.clients[num_clients].get_client_index();
        self.num_clients.set(num_clients+1);
        client.setup_client(self, retval);
        return ReturnCode::SUCCESS;
    }
    
    fn enable_clock(&self, cidx:&'static ClientIndex) -> Result<u32, ReturnCode> {
        let client_index = cidx.get_index();
        if client_index >= self.num_clients.get() {
            return Err(ReturnCode::EINVAL);
        }

        if self.clients[client_index].get_enabled() {
            self.clients[client_index].client_enabled();
            return Ok(self.configs.get_system_frequency());
        }

        self.clients[client_index].set_enabled(true);
        let client_clocks = self.clients[client_index].get_clockmask();
        let next_clockmask = self.change_clockmask.get() & client_clocks;

        // If no peripherals are running 
        // OR the current clock is incompatible
        // OR the requesting client can use a lower power clock than current clock
        let current_clock = self.current_clock.get();
        if (self.lock_count.get() == 0 && 
            (self.nolock_clockmask.get() > self.configs.get_all_clocks())) ||
            client_clocks & current_clock == 0 {
            //TODO is this condition necessary?
            //client_clocks % current_clock != 0 {
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

        return Ok(self.configs.get_system_frequency());
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
            let mut new_clockmask = 0xffffffff;
            for i in 0..num_clients { 
                if !self.clients[i].get_need_lock() &&
                        self.clients[i].get_running() {
                    new_clockmask &= self.clients[i].get_clockmask();
                }
            }
            self.nolock_clockmask.set(new_clockmask);
        }
        //self.clients[client_index].client_disabled();

        if self.lock_count.get() == 0 && !self.compute_mode.get() {
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
}
