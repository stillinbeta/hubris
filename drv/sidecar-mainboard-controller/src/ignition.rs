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
        Ok(v & 0x0000ffffffffffff)
    }

    pub fn counters(&self, id: u8) -> Result<u32, FpgaError> {
        self.read_raw::<u16, u32>(id, 0x10)
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
pub enum PowerState {
    Off = 0,
    On = 1,
}

bitfield! {
    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, FromBytes, AsBytes)]
    #[repr(C)]
    pub struct Request(u8);
    pub kind, set_kind: 1, 0;
    pub pending, _: 2;
}

impl From<PowerState> for Request {
    fn from(state: PowerState) -> Self {
        match state {
            PowerState::On => Request(0x01),
            PowerState::Off => Request(0x02),
        }
    }
}

impl Default for Request {
    fn default() -> Self {
        Self(0)
    }
}

bitfield! {
    #[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, FromBytes, AsBytes)]
    #[repr(C)]
    pub struct Response(u8);
    pub kind, _: 2, 0;
    pub valid, _: 3;
}

#[derive(
    Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive, AsBytes,
)]
#[repr(u8)]
pub enum Counter {
    TransmitterLost = 0,
    PacketsReceived = 1,
    PacketsDropped = 2,
}
