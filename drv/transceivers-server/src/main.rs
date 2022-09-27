// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

use drv_sidecar_front_io::transceivers::Transceivers;
use drv_transceivers_api::*;
use userlib::*;

task_slot!(FRONT_IO, front_io);

struct ServerImpl {
    transceivers: Transceivers,
}

impl idl::InOrderTransceiversImpl for ServerImpl {
    fn get_power_good(
        &mut self,
        _msg: &userlib::RecvMessage,
    ) -> Result<u32, idol_runtime::RequestError<TransceiversError>> {
        Ok(self
            .transceivers
            .get_power_good()
            .map_err(TransceiversError::from)?)
    }

    fn get_power_good_timeout(
        &mut self,
        _msg: &userlib::RecvMessage,
    ) -> Result<u32, idol_runtime::RequestError<TransceiversError>> {
        Ok(self
            .transceivers
            .get_power_good_timeout()
            .map_err(TransceiversError::from)?)
    }

    fn get_presence(
        &mut self,
        _msg: &userlib::RecvMessage,
    ) -> Result<u32, idol_runtime::RequestError<TransceiversError>> {
        Ok(self
            .transceivers
            .get_presence()
            .map_err(TransceiversError::from)?)
    }

    fn get_irq_rxlos(
        &mut self,
        _msg: &userlib::RecvMessage,
    ) -> Result<u32, idol_runtime::RequestError<TransceiversError>> {
        Ok(self
            .transceivers
            .get_irq_rxlos()
            .map_err(TransceiversError::from)?)
    }
}

#[export_name = "main"]
fn main() -> ! {
    loop {
        let mut buffer = [0; idl::INCOMING_SIZE];
        let transceivers = Transceivers::new(FRONT_IO.get_task_id());

        let mut server = ServerImpl { transceivers };

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
