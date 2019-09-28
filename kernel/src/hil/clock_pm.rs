use crate::returncode::ReturnCode;

/// Implemented by each peripheral
pub trait ClockClient {
    /// The ClockManager will call this function to report a clock change
    fn configure_clock(&self, frequency: u32);
    fn clock_enabled(&self);
    fn clock_disabled(&self);
}

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

pub trait ChangeClock {
    fn change_clock(&self) -> Option<u32>;
}

pub trait ClockManager {
    //TODO how to make this visible to ClockClients as well?
    type ClientIndex;

    fn register(&self, c:&'static ClockClient) -> Result<&'static Self::ClientIndex, ReturnCode>;
    fn enable_clock(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn disable_clock(&self, client_index:&'static Self::ClientIndex) -> ReturnCode;

    /// Accesssors for current ClockData state
    fn set_need_lock(&self, client_index:&'static Self::ClientIndex, need_lock: bool) -> ReturnCode;
    fn set_clocklist(&self, client_index:&'static Self::ClientIndex, clocklist: u32) -> ReturnCode;
    fn set_min_frequency(&self, client_index:&'static Self::ClientIndex, min_freq: u32) -> ReturnCode;
    fn set_max_frequency(&self, client_index:&'static Self::ClientIndex, max_freq: u32) -> ReturnCode;
    fn set_preferred(&self, client_index:&'static Self::ClientIndex, thresh_freq: u32) -> ReturnCode;

    fn get_need_lock(&self, client_index:&'static Self::ClientIndex) -> Result<bool, ReturnCode>;
    fn get_clocklist(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn get_min_frequency(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn get_max_frequency(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
    fn get_preferred(&self, client_index:&'static Self::ClientIndex) -> Result<u32, ReturnCode>;
}

