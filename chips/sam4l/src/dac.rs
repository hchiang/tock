// dac.rs -- Implementation of SAM4L DACC.
//
// This is a bare-bones implementation of the SAM4L DACC. 
// 
// DACC Features:
// 	-10-bit reslution
//	-internal trigger
//	-external and peripheral triggers (not implemented)
// 
// Author: Justin Hsieh <hsiehju@umich.edu>
// Date: May 26th, 2017

use core::cell::Cell;
use core::mem;
use kernel::common::volatile_cell::VolatileCell;
use kernel::hil;
use kernel::returncode::ReturnCode;
use pm::{self, Clock, PBAClock};
use nvic;

#[repr(C, packed)]
pub struct DacRegisters {
    // From page 905 of SAM4L manual
    cr: VolatileCell<u32>, 		// Control               			(0x00)
    mr: VolatileCell<u32>, 		// Mode        						(0x04)
    cdr: VolatileCell<u32>, 	// Conversion Data Register			(0x08)
    ier: VolatileCell<u32>, 	// Interrupt Enable Register        (0x0c)
    idr: VolatileCell<u32>, 	// Interrupt Disable Register		(0x10)
    imr: VolatileCell<u32>, 	// Interrupt Mask Register  		(0x14)
    isr: VolatileCell<u32>, 	// Interrupt Status Register        (0x18)
	_reserved0: [u32; 50],		//									(0x1c - 0xe0)
    wpmr: VolatileCell<u32>, 	// Write Protect Mode Register     	(0xe4)
    wpsr: VolatileCell<u32>, 	// Write Protect Status Register 	(0xe8)
    _reserved1: [u32; 4],		//									(0xec - 0xf8)
	version: VolatileCell<u32>, // Version Register       			(0xfc)
}

// Page 59 of SAM4L data sheet
const BASE_ADDRESS: *mut DacRegisters = 0x4003C000 as *mut DacRegisters;

pub struct Dac {
    registers: *mut DacRegisters,
    enabled: Cell<bool>,
}

pub static mut DAC: Dac = Dac::new(BASE_ADDRESS);

impl Dac {
    const fn new(base_address: *mut DacRegisters) -> Dac {
        Dac {
            registers: base_address,
            enabled: Cell::new(false),
        }
    }

    pub fn handle_interrupt(&mut self) {
		//debug!("Interrupt!");
		
        let regs: &mut DacRegisters = unsafe { mem::transmute(self.registers) };
		regs.cdr.set(0x567); 
	}
}

impl hil::dac::DacChannel for Dac {
    fn initialize(&self) -> ReturnCode {
        let regs: &mut DacRegisters = unsafe { mem::transmute(self.registers) };
        if !self.enabled.get() {
			self.enabled.set(true);
            
            ///  Start the APB clock (CLK_DACC)
            unsafe {
                pm::enable_clock(Clock::PBA(PBAClock::DACC));	
                nvic::enable(nvic::NvicIdx::DACC);
            }
			/// Reset DACC
			regs.cr.set(1);

            
			debug!("ISR: {}", regs.isr.get());
			
			/// Enable the DAC
            //let mut mr: u32 = regs.mr.get();
            //mr |= 1 << 4;
			debug!("ISR: {}", regs.isr.get());
			

			debug!("Mode Reg Before: {}", regs.mr.get());
			regs.mr.set(0x02000010);
			debug!("Mode Reg After: {}", regs.mr.get());

			regs.ier.set(1);

			debug!("ISR: {}", regs.isr.get());

			/// Set DAC value
			while regs.isr.get() == 1 {
				regs.cdr.set(567); 
			}

			debug!("ISR: {}", regs.isr.get());
			debug!("Version: {}", regs.version.get());
        }
		return ReturnCode::SUCCESS;
    }
    

    fn set_value(&self, value: u32) -> ReturnCode {
        let regs: &mut DacRegisters = unsafe { mem::transmute(self.registers) };
        if !self.enabled.get() {
            return ReturnCode::EOFF;
        } 
		else {
			
			/// Set DAC value
			while regs.isr.get() == 1 {
				regs.cdr.set(567); 
			}

			//regs.cdr.set(value);
            return ReturnCode::SUCCESS;
        }
    }

}


interrupt_handler!(dacc_handler, DACC);

