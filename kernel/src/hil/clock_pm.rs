use core::cell::Cell;
use tock_cells::optional_cell::OptionalCell;
use crate::returncode::ReturnCode;

/// Implemented by each peripheral
pub trait ClockClient {
    /// The ClockManager will call this function to report a clock change
    fn clock_enabled(&self);
    fn clock_disabled(&self);
}

pub trait ClockManager {
    //TODO how to make this visible to ClockClients as well?
    type ClientIndex;

    fn register(&self, c:&'static ClockClient) -> &'static Self::ClientIndex;
    fn enable_clock(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn disable_clock(&self, client_index:&'static Self::ClientIndex) -> ReturnCode;

    /// Accesssors for current ClockData state
    fn set_need_lock(&self, client_index:&'static Self::ClientIndex, need_lock: bool) -> ReturnCode;
    fn set_clocklist(&self, client_index:&'static Self::ClientIndex, clocklist: u32) -> ReturnCode;
    fn set_min_frequency(&self, client_index:&'static Self::ClientIndex, min_freq: u32) -> ReturnCode;
    fn set_max_frequency(&self, client_index:&'static Self::ClientIndex, max_freq: u32) -> ReturnCode;

    fn get_need_lock(&self, client_index:&'static Self::ClientIndex) -> Result<bool, ReturnCode>;
    fn get_clocklist(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn get_min_frequency(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn get_max_frequency(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
}

