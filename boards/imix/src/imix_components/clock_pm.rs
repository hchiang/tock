//! Component for imix board buttons.
//!
//! This provides one Component, ButtonComponent, which implements a
//! userspace syscall interface to the one imix on-board button (pin
//! 24).
//!
//! Usage
//! -----
//! ```rust
//! let button = ButtonComponent::new(board_kernel).finalize(());
//! ```

// Author: Holly Chiang <hchiang1@stanford.edu>
// Last modified: 12/10/2019

#![allow(dead_code)] // Components are intended to be conditionally included

use capsules::clock_pm;
use kernel::component::Component;
use kernel::static_init;

pub struct ClockManagerComponent {
    chip_configs: &'static dyn kernel::hil::clock_pm::ClockConfigs, 
}

impl ClockManagerComponent {
    pub fn new(chip_configs: &'static dyn kernel::hil::clock_pm::ClockConfigs) -> ClockManagerComponent {
        ClockManagerComponent {
            chip_configs: chip_configs,
        }
    }
}

impl Component for ClockManagerComponent {
    type StaticInput = ();
    type Output = &'static clock_pm::ClockManagement<'static>;

    unsafe fn finalize(&mut self, _s: Self::StaticInput) -> Self::Output {

        let all_clocks = self.chip_configs.get_all_clocks();
        let max_freq = self.chip_configs.get_max_freq();

        let clients = [
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX0, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX1, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX2, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX3, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX4, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX5, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX6, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX7, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX8, all_clocks, max_freq),
            clock_pm::ClockData::new(&clock_pm::CLIENT_INDEX9, all_clocks, max_freq),
        ];

        let clock_manager = static_init!(
            clock_pm::ClockManagement<'static>,
            clock_pm::ClockManagement::new(
                self.chip_configs,
                clients,
            )
        );

        clock_manager
    }
}
