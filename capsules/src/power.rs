pub trait PowerClient {
    /// The PowerManager will call this function to report a clock change
    fn clock_updated(&self, clock: u32);
}

pub trait PowerManager {
    fn register_client(&self, client: &'static PowerClient);

    /// Clients should call this function to update the PowerManager
    /// on which clocks they can tolerate
    fn report_acceptable_clocks(&self, clockmask: u32);
}
