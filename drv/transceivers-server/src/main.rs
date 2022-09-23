// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

use drv_fpga_api::*;
use drv_transceivers_api::*;
use drv_sidecar_front_io::transceivers::{Transceivers, self};
use idol_runtime::Server;
use userlib::*;

task_slot!(FRONT_IO, front_io);

struct ServerImpl {
    transceivers: Transceivers,
}

impl idl::InOrderTransceiversImpl for ServerImpl {
    fn read_presence(
        &mut self,
        msg: &userlib::RecvMessage,
    ) -> Result<u32,idol_runtime::RequestError<TransceiversError>> {
        Ok(self.transceivers.transceiver_presence().map_err(TransceiversError::from)?)
    }
}

#[export_name = "main"]
fn main() -> ! {
    loop {
        let mut buffer = [0; idl::INCOMING_SIZE];
        let transceivers = Transceivers::new(FRONT_IO.get_task_id());

        let mut server = ServerImpl {transceivers};

        loop {
            idol_runtime::dispatch(&mut buffer, &mut server);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

mod idl {
    use super::TransceiversError;

    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
