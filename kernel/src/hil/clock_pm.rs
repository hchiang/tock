use crate::returncode::ReturnCode;

pub struct ClientIndex {
    client_index: usize,
}

impl ClientIndex {
    pub const fn new(client_index: usize) -> ClientIndex {
        ClientIndex {
            client_index: client_index,
        }
    }
    pub fn get_index(&self) -> usize {
        self.client_index
    }
}

/// Chip specific implementations
pub trait ClockConfigs {
    fn get_num_clock_sources(&self) -> u32;
    fn get_max_freq(&self) -> u32;
    fn get_all_clocks(&self) -> u32;
    fn get_default(&self) -> u32;
    fn get_compute(&self) -> u32;
    
    fn get_clockmask(&self, min_freq: u32, max_freq: u32) -> u32;
    fn get_clock_frequency(&self, clock: u32) -> u32;
    fn get_system_frequency(&self) -> u32;
    fn change_system_clock(&self, clock:u32);
}

/// Implemented by each peripheral
pub trait ClockClient {
    /// The ClockManager will call this function to report a clock change
    fn setup_client(&self, clock_manager: &'static dyn ClockManager, client_index: &'static ClientIndex);
    fn configure_clock(&self, frequency: u32);
    fn clock_enabled(&self);
    fn clock_disabled(&self);
}

pub trait ClockManager {
    fn register(&'static self, c:&'static dyn ClockClient) -> ReturnCode;
    fn enable_clock(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn disable_clock(&self, client_index:&'static ClientIndex) -> ReturnCode;

    /// Accesssors for current ClockData state
    fn set_need_lock(&self, client_index:&'static ClientIndex, need_lock: bool) -> ReturnCode;
    fn set_clocklist(&self, client_index:&'static ClientIndex, clocklist: u32) -> ReturnCode;
    fn set_min_frequency(&self, client_index:&'static ClientIndex, min_freq: u32) -> ReturnCode;
    fn set_max_frequency(&self, client_index:&'static ClientIndex, max_freq: u32) -> ReturnCode;

    fn get_need_lock(&self, client_index:&'static ClientIndex) -> Result<bool, ReturnCode>;
    fn get_clocklist(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn get_min_frequency(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn get_max_frequency(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
}

pub trait ChangeClock {
    fn change_clock(&self);
    fn set_compute_mode(&self, compute_mode: bool);
}

