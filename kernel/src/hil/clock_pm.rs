use core::cell::Cell;
use common::cells::OptionalCell;
use returncode::ReturnCode;

/// Implemented by each peripheral
pub trait ClockClient {
    /// This function will by called by ClockManager's register function 
    ///     Indicates the peripheral should turn on clock management
    ///     The peripheral needs to keep track of client_index
    fn enable_cm(&self, client_index: &'static ClientIndex);
    /// The ClockManager will call this function to report a clock change
    fn clock_enabled(&self);
    fn clock_disabled(&self);
}

/// Data structure for peripherals to store clock management info
//pub struct ClockClientData {
//    enabled: Cell<bool>,
//    client_index: Cell<ClientIndex>,
//    lock: Cell<bool>,
//}
//
//impl ClockClientData {
//    pub const fn new(enabled: bool, client_index: usize, has_lock: bool) -> ClockClientData {
//        ClockClientData {
//            enabled: Cell::new(enabled),
//            client_index: Cell::new(client_index),
//            lock: Cell::new(has_lock),
//        }
//     }
//
//    pub fn enabled(&self) -> bool { self.enabled.get() }
//    pub fn client_index(&self) -> ClientIndex { self.client_index.get() }
//    pub fn has_lock(&self) -> bool { self.lock.get() }
//
//    pub fn set_enabled(&self, enabled: bool) { self.enabled.set(enabled) }
//    pub fn set_client_index(&self, client_index: ClientIndex) {
//        self.client_index.set(client_index); 
//    }
//    pub fn set_has_lock(&self, lock: bool) { self.lock.set(lock); }
//}

pub trait ClientIndex {
    fn get(&self) -> usize;
}

pub trait ClockManager<'a> {
    /// Clients should call this function to update the ClockManager
    /// on which clocks they can tolerate
    ///
    fn register(&self, c:&'a ClockClient);
    fn enable_clock(&self, client_index:&'static ClientIndex) -> ReturnCode;
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
