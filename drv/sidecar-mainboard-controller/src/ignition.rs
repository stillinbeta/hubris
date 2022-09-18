// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::MainboardController;
use bitfield::bitfield;
use drv_fpga_api::{FpgaError, FpgaUserDesign, WriteOp};
use userlib::{FromPrimitive, ToPrimitive};
use zerocopy::{AsBytes, FromBytes};

include!(concat!(env!("OUT_DIR"), "/ignition_controller.rs"));

pub struct IgnitionController {
    fpga: FpgaUserDesign,
    address_base: u16,
}

impl IgnitionController {
    pub fn new(task_id: userlib::TaskId, address_base: u16) -> Self {
        Self {
            fpga: FpgaUserDesign::new(
                task_id,
                MainboardController::DEVICE_INDEX,
            ),
            address_base,
        }
    }

    #[inline]
    fn addr<A>(&self, id: u8, addr: A) -> u16
    where
        u16: From<A>,
    {
        self.address_base + (256 * id as u16) + u16::from(addr)
    }

    fn read_raw<A, T>(&self, id: u8, addr: A) -> Result<T, FpgaError>
    where
        u16: From<A>,
        T: AsBytes + Default + FromBytes,
    {
        self.fpga.read(self.addr(id, addr))
    }

    fn write_raw<A, T>(
        &self,
        id: u8,
        addr: A,
        value: T,
    ) -> Result<(), FpgaError>
    where
        u16: From<A>,
        T: AsBytes + Default + FromBytes,
    {
        self.fpga.write(WriteOp::Write, self.addr(id, addr), value)
    }

    pub fn state(&self, id: u8) -> Result<u64, FpgaError> {
        let v: u64 = self.read_raw::<u16, u64>(id, 0x0)?;
        Ok(v & 0x00ffffffffffffff)
    }

    pub fn counters(&self, id: u8) -> Result<[u8; 4], FpgaError> {
        self.read_raw::<u16, [u8; 4]>(id, 0x10)
    }

    pub fn request(&self, id: u8) -> Result<u8, FpgaError> {
        self.read_raw::<u16, u8>(id, 0x8)
    }

    pub fn set_request(&self, id: u8, request: Request) -> Result<(), FpgaError> {
        self.write_raw::<u16, u8>(id, 0x8, request.to_u8().unwrap_or(0))
    }
}

bitfield! {
    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, FromBytes, AsBytes)]
    #[repr(C)]
    pub struct LinkStatus(u8);
    pub link_detected, _: 0;
    pub target_state_valid, _: 1;
}

bitfield! {
    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, FromBytes, AsBytes)]
    #[repr(C)]
    pub struct Target(u32);
    pub present, _: 0;
    pub valid, _: 1;
}

impl Target {
    pub fn system_type(&self) -> Option<SystemType> {
        if self.valid() {
            Some(SystemType((self.0 >> 8) as u8))
        } else {
            None
        }
    }

    pub fn status(&self) -> Option<Status> {
        if self.valid() {
            Some(Status((self.0 >> 16) as u8))
        } else {
            None
        }
    }

    pub fn power_state(&self) -> Option<PowerState> {
        self.status().map(|s| {
            if s.system_power_enabled() {
                PowerState::On
            } else {
                PowerState::Off
            }
        })
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    FromPrimitive,
    ToPrimitive,
    FromBytes,
    AsBytes,
)]
#[repr(C)]
pub struct SystemType(pub u8);

bitfield! {
    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, FromBytes, AsBytes)]
    #[repr(C)]
    pub struct Status(u8);
    pub controller_detected, _: 0;
    pub system_power_enabled, _: 1;
    pub power_fault_a3, _: 2;
    pub power_fault_a2, _: 3;
    pub rot_fault, _: 4;
    pub sp_fault, _: 5;
}

#[derive(
    Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, AsBytes,
)]
#[repr(u8)]
pub enum Request {
    SystemPowerOff = 1,
    SystemPowerOn = 2,
    SystemReset = 3,
}

#[derive(
    Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, AsBytes,
)]
#[repr(u8)]
pub enum PowerState {
    Off = 0,
    On = 1,
}
