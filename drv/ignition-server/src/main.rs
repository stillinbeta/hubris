// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Server for interacting with Ignition Controllers.

#![no_std]
#![no_main]

use drv_fpga_api::*;
use drv_ignition_api::*;
use drv_sidecar_mainboard_controller::ignition::*;
use ringbuf::*;
use userlib::*;

task_slot!(FPGA, fpga);

#[derive(Copy, Clone, Debug, PartialEq)]
enum Trace {
    None,
    ReadTarget(u8, Target),
    ReadRequest(u8, Request),
    SetRequest(u8, Request),
    ReadResponse(u8, Response),
    SetPowerState(u8, PowerState),
}
ringbuf!(Trace, 16, Trace::None);

#[export_name = "main"]
fn main() -> ! {
    let mut incoming = [0u8; idl::INCOMING_SIZE];
    let mut server = ServerImpl {
        controller: IgnitionController::new(FPGA.get_task_id(), 0x100),
    };

    loop {
        idol_runtime::dispatch(&mut incoming, &mut server);
    }
}

struct ServerImpl {
    controller: IgnitionController,
}

impl ServerImpl {
    /*
    fn set_request_read_response(
        &self,
        id: u8,
        request: Request,
    ) -> Result<(), RequestError> {
        self.controller
            .set_request(id, request)
            .map_err(IgnitionError::from)
            .map_err(RequestError::from)?;

        let mut i = 3;
        let mut response = Response(0);

        while !response.valid() && i > 0 {
            i -= 1;
            response = self
                .controller
                .response(id)
                .map_err(IgnitionError::from)
                .map_err(RequestError::from)?;

            hl::sleep_for(5);
        }

        match response {
            Response(0x8) => Ok(()),
            Response(0x9) => Err(RequestError::from(IgnitionError::Nack)),
            _ if response.valid() => {
                Err(RequestError::from(IgnitionError::InvalidValue))
            }
            _ => Err(RequestError::from(IgnitionError::Timeout)),
        }
    }
    */
}

type RequestError = idol_runtime::RequestError<IgnitionError>;

impl idl::InOrderIgnitionImpl for ServerImpl {
    fn state(
        &mut self,
        _: &userlib::RecvMessage,
        id: u8,
    ) -> Result<u64, RequestError> {
        self.controller
            .state(id)
            .map_err(IgnitionError::from)
            .map_err(RequestError::from)
    }

    fn counters(
        &mut self,
        _: &userlib::RecvMessage,
        id: u8,
    ) -> Result<u32, RequestError> {
        self.controller
            .counters(id)
            .map_err(IgnitionError::from)
            .map_err(RequestError::from)
    }
}

mod idl {
    use drv_ignition_api::*;
    use drv_sidecar_mainboard_controller::ignition::*;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
