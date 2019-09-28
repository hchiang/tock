use kernel::{AppId, Callback, Driver, ReturnCode};
use kernel::hil::clock_pm::{ChangeClock};

pub struct ClockCM<'a, C:ChangeClock> {
    clock_manager: &'a C,
}

impl<C: ChangeClock> ClockCM<'a, C> {
    pub fn new(clock_manager: &'a C) -> ClockCM<'a, C> {
        ClockCM {
            clock_manager: clock_manager,
        }
    }
}

impl<C: ChangeClock> Driver for ClockCM<'a, C> {

    fn subscribe(
        &self, 
        subscribe_num: usize, 
        _callback: Option<Callback>, 
        _app_id: AppId,
    ) -> ReturnCode {
        match subscribe_num {
            // default
            _ => ReturnCode::ENOSUPPORT,
        }
    }

    fn command(&self, command_num: usize, _: usize, _: usize, _: AppId) -> ReturnCode {
        match command_num {
            // number of pins
            1 => {
                self.clock_manager.change_clock();
                ReturnCode::SUCCESS
            },
            _ => ReturnCode::ENOSUPPORT,
        }
    }
}
