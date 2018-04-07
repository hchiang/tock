use kernel::{AppId, Callback, Driver, ReturnCode};
use kernel::hil::clock_pm::{SetClock};


pub struct ClockCM<C: SetClock> {
    clock_manager: C,
}

impl<C: SetClock> ClockCM<C> {
    pub fn new(clock_manager: C) -> ClockCM<C> {
        ClockCM {
            clock_manager: clock_manager,
        }
    }
}

impl<C: SetClock> Driver for ClockCM<C> {

    fn subscribe(&self, subscribe_num: usize, _: Callback) -> ReturnCode {
        match subscribe_num {
            // default
            _ => ReturnCode::ENOSUPPORT,
        }
    }

    fn command(&self, command_num: usize, clock: usize, _: usize, _: AppId) -> ReturnCode {
        match command_num {
            // number of pins
            0 => {
                self.clock_manager.set_clock(clock as u32);
                ReturnCode::SUCCESS
            },
            _ => ReturnCode::ENOSUPPORT,
        }
    }
}

