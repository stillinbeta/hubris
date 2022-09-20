// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]
#![no_main]

use drv_fpga_api::*;
use drv_transceivers_api::*;
use userlib::*;

task_slot!(FPGA, fpga);

#[export_name = "main"]
fn main() -> ! {
    loop {
        let mut buffer = [0; idl::INCOMING_SIZE];
        let mut server = ServerImpl {};
    
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