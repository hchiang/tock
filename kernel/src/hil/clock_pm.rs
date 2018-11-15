use core::cell::Cell;
use common::cells::OptionalCell;
use returncode::ReturnCode;

pub trait ClockClient {
    /// This function will by called by ClockManager's register function 
    ///     Indicates the peripheral should turn on clock management
    fn enable_cm(&self, client_index: usize);
    /// The ClockManager will call this function to report a clock change
    fn clock_updated(&self);
}

pub struct ClockClientData {
    enabled: Cell<bool>,
    client_index: Cell<usize>,
    lock: Cell<bool>,
    //clock_params: ClockParams,
}
impl ClockClientData {
    pub const fn new(enabled: bool, client_index: usize, has_lock: bool) -> ClockClientData {
        ClockClientData {
            enabled: Cell::new(enabled),
            client_index: Cell::new(client_index),
            lock: Cell::new(has_lock),
        }
     }

    pub fn enabled(&self) -> bool { self.enabled.get() }
    pub fn client_index(&self) -> usize { self.client_index.get() }
    pub fn has_lock(&self) -> bool { self.lock.get() }

    pub fn set_enabled(&self, enabled: bool) { self.enabled.set(enabled) }
    pub fn set_client_index(&self, client_index: usize) {
        self.client_index.set(client_index); 
    }
    pub fn set_has_lock(&self, lock: bool) { self.lock.set(lock); }
}

pub struct ClockData<'a> {
    client: OptionalCell<&'a ClockClient>,
    enabled: Cell<bool>,
    //need_lock: Cell<bool>,
    clockmask: Cell<u32>,
    clocklist: Cell<u32>,
    min_freq: Cell<u32>,
    max_freq: Cell<u32>,
}
impl ClockData<'a>{
    pub const fn new() -> ClockData<'a> {
        ClockData{
            client: OptionalCell::empty(),
            enabled: Cell::new(false),
            clockmask: Cell::new(0x3ff),
            clocklist: Cell::new(0x3ff),
            min_freq: Cell::new(0),
            max_freq: Cell::new(48000000),
        }
    }
    pub fn initialize(&self, client: &'a ClockClient) {
        self.client.set(client);
        self.enabled.set(false);
        self.clockmask.set(0x3ff);
        self.clocklist.set(0x3ff);
        self.min_freq.set(0);
        self.max_freq.set(48000000);
    }

    pub fn client_update(&self) {
        let client = self.client.take();
        match client {
            Some(clock_client) => {
                clock_client.clock_updated();
                self.client.set(clock_client);
            },
            None => {},
        }
    }
    pub fn get_enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn get_clockmask(&self) -> u32 {
        self.clockmask.get()
    }
    pub fn get_clocklist(&self) -> u32 {
        self.clocklist.get()
    }
    pub fn get_min_freq(&self) -> u32 {
        self.min_freq.get()
    }
    pub fn get_max_freq(&self) -> u32 {
        self.max_freq.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
    pub fn set_clockmask(&self, clockmask: u32) {
        self.clockmask.set(clockmask);
    }
    pub fn set_clocklist(&self, clocklist: u32) {
        self.clocklist.set(clocklist);
    }
    pub fn set_min_freq(&self, min_freq: u32) {
        self.min_freq.set(min_freq);
    }
    pub fn set_max_freq(&self, max_freq: u32) {
        self.max_freq.set(max_freq);
    }
}

pub trait ClockManager<'a> {
    /// Clients should call this function to update the ClockManager
    /// on which clocks they can tolerate
    ///
    fn register(&self, c:&'a ClockClient);
    fn enable_clock(&self, client_index: usize) -> ReturnCode;
    fn disable_clock(&self, client_index: usize) -> ReturnCode;

    /// Accesssors for current ClockParams state
    fn set_clocklist(&self, client_index: usize, clocklist: u32) -> ReturnCode;
    fn set_min_frequency(&self, client_index: usize, min_freq: u32) -> ReturnCode;
    fn set_max_frequency(&self, client_index: usize, max_freq: u32) -> ReturnCode;

    fn get_clocklist(&self, client_index: usize) -> Result<u32, ReturnCode>;
    fn get_min_frequency(&self, client_index: usize) -> Result<u32, ReturnCode>;
    fn get_max_frequency(&self, client_index: usize) -> Result<u32, ReturnCode>;
}
