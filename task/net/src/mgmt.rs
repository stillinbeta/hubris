// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::miim_bridge::MiimBridge;
use drv_spi_api::{SpiDevice, SpiError};
use drv_stm32h7_eth::Ethernet;
use drv_stm32xx_sys_api::{self as sys_api, OutputType, Pull, Speed, Sys};
use ksz8463::{Ksz8463, Register as KszRegister};
use ringbuf::*;
use userlib::hl::sleep_for;
use vsc7448_pac::phy;
use vsc85xx::{vsc85x2::Vsc85x2, Counter, VscError};

/// On some boards, the KSZ8463 reset line is tied to an RC + diode network
/// which dramatically slows its rise and fall times.  We use this parameter
/// to mark this case and handle it separately.
///
/// This is flagged with allow(dead_code) because each BSP may only use one
/// or the other behavior, and we only compile one BSP at a time.
#[allow(dead_code)]
pub enum Ksz8463ResetSpeed {
    Slow,
    Normal,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Status {
    ksz8463_100base_fx_link_up: [bool; 2],
    ksz8463_rx_bytes: [ksz8463::MIBCounterValue; 2],
    ksz8463_tx_bytes: [ksz8463::MIBCounterValue; 2],

    vsc85x2_100base_fx_link_up: [bool; 2],
    vsc85x2_sgmii_link_up: [bool; 2],

    // The VSC8562 includes MAC TX/RX counters as well, but these
    // aren't present on the VSC8552.
    vsc85x2_media_tx_good_count: [Counter; 2],
    vsc85x2_mac_tx_good_count: [Counter; 2],
    vsc85x2_media_rx_good_count: [Counter; 2],
    vsc85x2_mac_rx_good_count: [Counter; 2],
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Trace {
    None,
    Ksz8463Err { port: u8, err: SpiError },
    Vsc85x2Err { port: u8, err: VscError },
    Status(Status),
}

ringbuf!(Trace, 16, Trace::None);

/// Configuration struct for the rest of the management network hardware,
/// which is a KSZ8463 switch attached to a VSC8552 or VSC8562 PHY.
pub struct Config {
    /// Controls power to the management network
    pub power_en: Option<sys_api::PinSet>,

    /// Goes high once power is good
    pub power_good: Option<sys_api::PinSet>,

    /// Goes high once the PLLs are locked
    pub pll_lock: Option<sys_api::PinSet>,

    pub ksz8463_spi: SpiDevice,
    pub ksz8463_nrst: sys_api::PinSet,
    pub ksz8463_rst_type: Ksz8463ResetSpeed,

    pub vsc85x2_coma_mode: Option<sys_api::PinSet>,
    pub vsc85x2_nrst: sys_api::PinSet,
    pub vsc85x2_base_port: u8,
}

impl Config {
    pub fn build(self, sys: &Sys, eth: &mut Ethernet) -> Bsp {
        // The VSC8552 connects the KSZ switch to the management network
        // over SGMII
        let vsc85x2 = self.configure_vsc85x2(sys, eth);

        // The KSZ8463 connects to the SP over RMII, then sends data to the
        // VSC8552 over 100-BASE FX
        let ksz8463 = self.configure_ksz8463(sys);

        Bsp { ksz8463, vsc85x2 }
    }

    fn configure_ksz8463(self, sys: &Sys) -> ksz8463::Ksz8463 {
        sys.gpio_reset(self.ksz8463_nrst).unwrap();
        sys.gpio_configure_output(
            self.ksz8463_nrst,
            OutputType::PushPull,
            Speed::Low,
            Pull::None,
        )
        .unwrap();

        // Toggle the reset line
        sleep_for(10); // Reset must be held low for 10 ms after power up
        sys.gpio_set(self.ksz8463_nrst).unwrap();

        // The datasheet recommends a particular combination of diodes and
        // capacitors which dramatically slow down the rise of the reset
        // line, meaning you have to wait for extra long here.
        //
        // Otherwise, the minimum wait time is 1 µs, so 1 ms is fine.
        sleep_for(match self.ksz8463_rst_type {
            Ksz8463ResetSpeed::Slow => 150,
            Ksz8463ResetSpeed::Normal => 1,
        });

        let ksz8463 = Ksz8463::new(self.ksz8463_spi);

        // The KSZ8463 connects to the SP over RMII, then sends data to the
        // VSC8552 over 100-BASE FX
        ksz8463.configure();
        ksz8463
    }

    fn configure_vsc85x2(&self, sys: &Sys, eth: &mut Ethernet) -> Vsc85x2 {
        // TODO: wait for PLL lock to happen here

        // Start with reset low and COMA_MODE high
        sys.gpio_reset(self.vsc85x2_nrst).unwrap();
        sys.gpio_configure_output(
            self.vsc85x2_nrst,
            OutputType::PushPull,
            Speed::Low,
            Pull::None,
        )
        .unwrap();

        if let Some(coma_mode) = self.vsc85x2_coma_mode {
            sys.gpio_set(coma_mode).unwrap();
            sys.gpio_configure_output(
                coma_mode,
                OutputType::PushPull,
                Speed::Low,
                Pull::None,
            )
            .unwrap();
        }

        // Do a hard reset of power, if that's present on this board
        if let Some(power_en) = self.power_en {
            sys.gpio_reset(power_en).unwrap();
            sys.gpio_configure_output(
                power_en,
                OutputType::PushPull,
                Speed::Low,
                Pull::None,
            )
            .unwrap();
            sys.gpio_reset(power_en).unwrap();
            sleep_for(10); // TODO: how long does this need to be?

            // Power on
            sys.gpio_set(power_en).unwrap();
            sleep_for(4);
        }

        // TODO: sleep for PG lines going high here

        sys.gpio_set(self.vsc85x2_nrst).unwrap();
        sleep_for(120); // Wait for the chip to come out of reset

        // Build handle for the VSC85x2 PHY, then initialize it
        let rw = &mut MiimBridge::new(eth);
        let vsc85x2 = Vsc85x2::init(self.vsc85x2_base_port, rw);

        // Disable COMA_MODE
        if let Some(coma_mode) = self.vsc85x2_coma_mode {
            sys.gpio_reset(coma_mode).unwrap();
        }

        vsc85x2.unwrap() // TODO
    }
}

pub struct Bsp {
    pub ksz8463: Ksz8463,
    pub vsc85x2: Vsc85x2,
}

impl Bsp {
    pub fn wake(&self, eth: &mut Ethernet) {
        let mut s = Status::default();
        let rw = &mut MiimBridge::new(eth);
        for i in 0..2 {
            // The KSZ8463 numbers its ports starting at 1 (e.g. P1MBSR)
            let port = i as u8 + 1;
            match self.ksz8463.read(KszRegister::PxMBSR(port)) {
                Ok(sr) => {
                    s.ksz8463_100base_fx_link_up[i] = (sr & (1 << 2)) != 0
                }
                Err(err) => {
                    ringbuf_entry!(Trace::Ksz8463Err { port, err });
                    return;
                }
            }
            match self
                .ksz8463
                .read_mib_counter(port, ksz8463::MIBCounter::RxLoPriorityByte)
            {
                Ok(c) => s.ksz8463_rx_bytes[i] = c,
                Err(err) => {
                    ringbuf_entry!(Trace::Ksz8463Err { port, err });
                    return;
                }
            }
            match self
                .ksz8463
                .read_mib_counter(port, ksz8463::MIBCounter::TxLoPriorityByte)
            {
                Ok(c) => s.ksz8463_tx_bytes[i] = c,
                Err(err) => {
                    ringbuf_entry!(Trace::Ksz8463Err { port, err });
                    return;
                }
            }

            // The VSC85x2 numbers its ports starting at 0
            let port = i as u8;
            let mut phy = self.vsc85x2.phy(port, rw);
            match phy.read(phy::STANDARD::MODE_STATUS()) {
                Ok(sr) => {
                    s.vsc85x2_100base_fx_link_up[i] = (sr.0 & (1 << 2)) != 0
                }
                Err(err) => {
                    ringbuf_entry!(Trace::Vsc85x2Err { port, err });
                    return;
                }
            };
            match phy.read(phy::EXTENDED_3::MAC_SERDES_PCS_STATUS()) {
                Ok(status) => {
                    s.vsc85x2_sgmii_link_up[i] = (status.0 & (1 << 2)) != 0
                }
                Err(err) => {
                    ringbuf_entry!(Trace::Vsc85x2Err { port, err });
                    return;
                }
            };

            // Read media (100BASE-FX) and MAC counters, which are
            // chip-dependent (some aren't present on the VSC8552)
            match phy.media_tx_rx_good() {
                Ok((tx, rx)) => {
                    s.vsc85x2_media_tx_good_count[i] = tx;
                    s.vsc85x2_media_rx_good_count[i] = rx;
                }
                Err(err) => ringbuf_entry!(Trace::Vsc85x2Err { port, err }),
            }

            match phy.mac_tx_rx_good() {
                Ok((tx, rx)) => {
                    s.vsc85x2_mac_tx_good_count[i] = tx;
                    s.vsc85x2_mac_rx_good_count[i] = rx;
                }
                Err(err) => ringbuf_entry!(Trace::Vsc85x2Err { port, err }),
            }
        }
        ringbuf_entry!(Trace::Status(s));
    }
}