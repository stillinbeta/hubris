// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! API crate for QSFP transceiver managment

#![no_std]

use drv_fpga_api::FpgaError;
use derive_idol_err::IdolError;
use userlib::*;

#[derive(Copy, Clone, Debug, FromPrimitive, Eq, PartialEq, IdolError)]
pub enum TransceiversError {
    FpgaError = 1,
    InvalidPortNumber = 2,
    InvalidNumberOfBytes = 3,
}

impl From<FpgaError> for TransceiversError {
    fn from(_: FpgaError) -> Self {
        Self::FpgaError
    }
}

include!(concat!(env!("OUT_DIR"), "/client_stub.rs"));
