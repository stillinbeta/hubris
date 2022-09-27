// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Addr, Reg};
use drv_fpga_api::{FpgaError, FpgaUserDesign, WriteOp};
use zerocopy::{byteorder, AsBytes, FromBytes, Unaligned, U16, U32};

pub struct Transceivers {
    fpgas: [FpgaUserDesign; 2],
}

impl Transceivers {
    pub fn new(fpga_task: userlib::TaskId) -> Self {
        Self {
            // There are 16 QSFP-DD transceivers connected to each FPGA
            fpgas: [
                FpgaUserDesign::new(fpga_task, 0),
                FpgaUserDesign::new(fpga_task, 1),
            ],
        }
    }

    pub fn get_presence(&self) -> Result<u32, FpgaError> {
        let f0: u16 =
            u16::from_be(self.fpgas[0].read(Addr::QSFP_STATUS_PRESENT_L)?);
        let f1: u16 =
            u16::from_be(self.fpgas[1].read(Addr::QSFP_STATUS_PRESENT_L)?);
        Ok((f1 as u32) << 16 | (f0 as u32))
    }

    pub fn get_power_good(&self) -> Result<u32, FpgaError> {
        let f0: u16 = u16::from_be(self.fpgas[0].read(Addr::QSFP_STATUS_PG_L)?);
        let f1: u16 = u16::from_be(self.fpgas[1].read(Addr::QSFP_STATUS_PG_L)?);
        Ok((f1 as u32) << 16 | (f0 as u32))
    }

    pub fn get_power_good_timeout(&self) -> Result<u32, FpgaError> {
        let f0: u16 =
            u16::from_be(self.fpgas[0].read(Addr::QSFP_STATUS_PG_TIMEOUT_L)?);
        let f1: u16 =
            u16::from_be(self.fpgas[1].read(Addr::QSFP_STATUS_PG_TIMEOUT_L)?);
        Ok((f1 as u32) << 16 | (f0 as u32))
    }

    pub fn get_irq_rxlos(&self) -> Result<u32, FpgaError> {
        let f0: u16 =
            u16::from_be(self.fpgas[0].read(Addr::QSFP_STATUS_IRQ_L)?);
        let f1: u16 =
            u16::from_be(self.fpgas[1].read(Addr::QSFP_STATUS_IRQ_L)?);
        Ok((f1 as u32) << 16 | (f0 as u32))
    }

    pub fn setup_i2c_read(
        &self,
        reg: u8,
        num_bytes: u8,
        port_bcast_mask: u32,
    ) -> Result<(), FpgaError> {
        let bcast_mask: U32<byteorder::BigEndian> = U32::new(port_bcast_mask);
        let request = TransceiversI2CRequest {
            reg,
            num_bytes,
            port_bcast_mask: U16::new(bcast_mask.get() as u16),
            op: ((TransceiverI2COperation::RandomRead as u8) << 1)
                | Reg::QSFP::I2C_CTRL::START,
        };
        self.fpgas[0].write(
            WriteOp::Write,
            Addr::QSFP_I2C_REG_ADDR,
            request,
        )?;

        let request = TransceiversI2CRequest {
            reg,
            num_bytes,
            port_bcast_mask: U16::new((bcast_mask.get() >> 16) as u16),
            op: ((TransceiverI2COperation::RandomRead as u8) << 1)
                | Reg::QSFP::I2C_CTRL::START,
        };
        self.fpgas[1].write(
            WriteOp::Write,
            Addr::QSFP_I2C_REG_ADDR,
            request,
        )?;

        Ok(())
    }

    pub fn get_i2c_read_buffer(&self, port: usize, buf: &mut [u8]) -> Result<(), FpgaError> {
        let fpga_idx: usize = if port < 16 {0} else {1};
        //TODO: need to set the PORTx_READ_BUFFER dynamically
        self.fpgas[fpga_idx].read_bytes(Addr::QSFP_PORT0_READ_BUFFER, buf)
    }
}

#[derive(AsBytes)]
#[repr(u8)]
pub enum TransceiverI2COperation {
    Read = 0,
    Write = 1,
    // Start a Write to set the reg addr, then Start again to do read at that addr
    RandomRead = 2,
}

impl From<TransceiverI2COperation> for u8 {
    fn from(op: TransceiverI2COperation) -> Self {
        op as u8
    }
}

#[derive(AsBytes, FromBytes, Unaligned)]
#[repr(C)]
pub struct TransceiversI2CRequest {
    reg: u8,
    num_bytes: u8,
    port_bcast_mask: U16<byteorder::BigEndian>,
    op: u8,
}
