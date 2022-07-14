// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Server for interacting with Ignition Controllers.

#![no_std]
#![no_main]

use ringbuf::*;
use userlib::*;
//use zerocopy::{byteorder, AsBytes, Unaligned, U16};

#[derive(Copy, Clone, Debug, PartialEq)]
enum Trace {
    None,
}
ringbuf!(Trace, 16, Trace::None);

#[export_name = "main"]
fn main() -> ! {
    let mut incoming = [0u8; idl::INCOMING_SIZE];
    let mut server = ServerImpl {};

    loop {
        idol_runtime::dispatch(&mut incoming, &mut server);
    }
}

struct ServerImpl {}

impl idl::InOrderIgnitionImpl for ServerImpl {}

mod idl {
    include!(concat!(env!("OUT_DIR"), "/server_stub.rs"));
}
