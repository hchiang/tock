use capsules;
use core::cell::Cell;

pub struct ImixPowerManager {
    client: Cell<Option<&'static capsules::power::PowerClient>>,
    acceptable_clocks: Cell<u32>,
}

impl ImixPowerManager {
    pub fn new() -> Self {
        ImixPowerManager {
            client: Cell::new(None),
            acceptable_clocks: Cell::new(0xffffffff),
        }
    }

    fn choose_clock(&self) -> u32 {
        // Really we would choose some clock here
        self.acceptable_clocks.get()
    }

    pub fn update_clock(&self) {
        let clock = self.choose_clock();

        self.client.get().map(|c| { c.clock_updated(clock) });
    }
}

impl capsules::power::PowerManager for ImixPowerManager {
    fn register_client(&self, c: &'static capsules::power::PowerClient) {
        self.client.set(Some(c));
    }

    fn report_acceptable_clocks(&self, clockmask: u32) {
        let mask = self.acceptable_clocks.get() & clockmask;
        self.acceptable_clocks.set(mask);

        self.update_clock();
    }
}
