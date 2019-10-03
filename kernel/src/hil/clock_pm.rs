pub trait SetClock {
    fn set_clock(&self,clock:u32);
    fn start_timer(&self);
    fn get_timer(&self);
}
