use crate::returncode::ReturnCode;

//TODO: make ClientIndex type/generic so that it can't be created outside of ClockManager
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

/// Implemented by each peripheral
pub trait ClockClient {
    /// The ClockManager will call this function to report a clock change
    fn set_client_index(&self, client_index: &'static ClientIndex);
    fn configure_clock(&self, frequency: u32);
    fn clock_enabled(&self);
    fn clock_disabled(&self);
}

pub trait ClockManager {
    //TODO how to make this visible to ClockClients as well?
    fn register(&self, c:&'static ClockClient) -> ReturnCode;
    fn enable_clock(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn disable_clock(&self, client_index:&'static ClientIndex) -> ReturnCode;

    /// Accesssors for current ClockData state
    fn set_need_lock(&self, client_index:&'static ClientIndex, need_lock: bool) -> ReturnCode;
    fn set_clocklist(&self, client_index:&'static ClientIndex, clocklist: u32) -> ReturnCode;
    fn set_min_frequency(&self, client_index:&'static ClientIndex, min_freq: u32) -> ReturnCode;
    fn set_max_frequency(&self, client_index:&'static ClientIndex, max_freq: u32) -> ReturnCode;
    fn set_preferred(&self, client_index:&'static ClientIndex, thresh_freq: u32) -> ReturnCode;

    fn get_need_lock(&self, client_index:&'static ClientIndex) -> Result<bool, ReturnCode>;
    fn get_clocklist(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn get_min_frequency(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn get_max_frequency(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
    fn get_preferred(&self, client_index:&'static ClientIndex) -> Result<u32, ReturnCode>;
}

pub trait ChangeClock {
    fn change_clock(&self);
    fn set_compute_mode(&self, compute_mode: bool);
}

