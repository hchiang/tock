use core::cell::Cell;

pub trait ClockClient {
    /// This function will by called by ClockManager's register function 
    ///     Indicates the peripheral should turn on clock management
    fn enable_cm(&self, client_index: usize);
    /// The ClockManager will call this function to report a clock change
    fn clock_updated(&self);
}

pub struct ClockClientData {
    pub cm_enabled: Cell<bool>,
    pub client_index: Cell<usize>,
    pub has_lock: Cell<bool>,
    pub clock_params: ClockParams,
}

pub struct ClockParams {
    /// clocklist: bitmask of clocks the client can operate with
    pub clocklist: Cell<u32>, 
    /// min_freq: minimum operational frequency
    pub min_frequency: Cell<u32>, 
    /// max_freq: maximum operational frequency
    pub max_frequency: Cell<u32>, 
}

impl ClockParams {
    pub const fn new(clocklist: u32, min_frequency: u32, 
        max_frequency: u32) -> ClockParams{
        ClockParams{
            clocklist: Cell::new(clocklist),
            min_frequency: Cell::new(min_frequency),
            max_frequency: Cell::new(max_frequency),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ClockData<'a> {
    pub client: Option<&'a ClockClient>,
    pub enabled: bool,
    pub clock_mask: u32,
    pub min_freq: u32,
    pub max_freq: u32,
}

pub trait ClockManager<'a> {
    /// Clients should call this function to update the ClockManager
    /// on which clocks they can tolerate
    ///
    fn register(&mut self, c:&'a ClockClient);
    fn unlock(&mut self, client_index: usize);
    fn clock_change(&mut self, client_index: usize, params: &ClockParams);
}
