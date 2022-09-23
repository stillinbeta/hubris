// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Addr, Reg};
use drv_fpga_api::{FpgaError, FpgaUserDesign, WriteOp};

pub struct Transceivers {
    fpgas: [FpgaUserDesign; 2],
    await_not_busy_sleep_for: u64,
}

impl Transceivers {
    pub fn new(fpga_task: userlib::TaskId) -> Self {
        Self {
            // There are 16 QSFP-DD transceivers connected to each FPGA
            fpgas: [
                FpgaUserDesign::new(fpga_task, 0),
                FpgaUserDesign::new(fpga_task, 1),
            ],
            await_not_busy_sleep_for: 0,
        }
    }

    #[inline]
    pub fn transceiver_presence(&self) -> Result<u32, FpgaError> {
        let fpga0: u16 = u16::from_be(self.fpgas[0].read(Addr::QSFP_STATUS_PRESENT_L)?);
        let fpga1: u16 = u16::from_be(self.fpgas[1].read(Addr::QSFP_STATUS_PRESENT_L)?);
        let f0: u32 = fpga0 as u32;
        let f1: u32 = fpga1 as u32;
        Ok((f1 << 16) | f0)
    }
}
