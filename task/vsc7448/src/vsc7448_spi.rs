use drv_spi_api::{SpiDevice};
use ringbuf::*;
use vsc7448_pac::{
    phy,
    types::{PhyRegisterAddress, RegisterAddress},
    Vsc7448,
};
use crate::VscError;

#[derive(Copy, Clone, PartialEq)]
enum Trace {
    None,
    Read {
        addr: u32,
        value: u32,
    },
    Write {
        addr: u32,
        value: u32,
    },
    MiimSetPage {
        miim: u8,
        phy: u8,
        page: u16,
    },
    MiimRead {
        miim: u8,
        phy: u8,
        page: u16,
        addr: u8,
        value: u16,
    },
    MiimWrite {
        miim: u8,
        phy: u8,
        page: u16,
        addr: u8,
        value: u16,
    },
    MiimIdleWait,
    MiimReadWait,
}

// Flags to tune ringbuf output while developing
const DEBUG_TRACE_SPI: u8 = 1 << 0;
const DEBUG_TRACE_MIIM: u8 = 1 << 1;
const DEBUG_MASK: u8 = 0;

/// Writes the given value to the ringbuf if allowed by the global `DEBUG_MASK`
macro_rules! ringbuf_entry_masked {
    ($mask:ident, $value:expr) => {
        if (DEBUG_MASK & $mask) != 0 {
            ringbuf_entry!($value);
        }
    };
}
ringbuf!(Trace, 16, Trace::None);

////////////////////////////////////////////////////////////////////////////////

/// Helper struct to read and write from the VSC7448 over SPI
pub struct Vsc7448Spi(pub SpiDevice);
impl Vsc7448Spi {
    /// Reads from a VSC7448 register
    pub fn read<T>(&self, reg: RegisterAddress<T>) -> Result<T, VscError>
    where
        T: From<u32>,
    {
        assert!(reg.addr >= 0x71000000);
        assert!(reg.addr <= 0x72000000);
        let addr = (reg.addr & 0x00FFFFFF) >> 2;
        let data: [u8; 3] = [
            ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
        ];

        // We read back 8 bytes in total:
        // - 3 bytes of address
        // - 1 byte of padding
        // - 4 bytes of data
        let mut out = [0; 8];
        self.0.exchange(&data[..], &mut out[..])?;
        let value = (out[7] as u32)
            | ((out[6] as u32) << 8)
            | ((out[5] as u32) << 16)
            | ((out[4] as u32) << 24);

        ringbuf_entry_masked!(
            DEBUG_TRACE_SPI,
            Trace::Read {
                addr: reg.addr,
                value
            }
        );
        if value == 0x88888888 {
            panic!("suspicious read");
        }
        Ok(value.into())
    }

    /// Writes to a VSC7448 register.  This will overwrite the entire register;
    /// if you want to modify it, then use [Self::modify] instead.
    pub fn write<T>(
        &self,
        reg: RegisterAddress<T>,
        value: T,
    ) -> Result<(), VscError>
    where
        u32: From<T>,
    {
        assert!(reg.addr >= 0x71000000);
        assert!(reg.addr <= 0x72000000);

        let addr = (reg.addr & 0x00FFFFFF) >> 2;
        let value: u32 = value.into();
        let data: [u8; 7] = [
            0x80 | ((addr >> 16) & 0xFF) as u8,
            ((addr >> 8) & 0xFF) as u8,
            (addr & 0xFF) as u8,
            ((value >> 24) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        ];

        ringbuf_entry_masked!(
            DEBUG_TRACE_SPI,
            Trace::Write {
                addr: reg.addr,
                value: value.into()
            }
        );
        self.0.write(&data[..])?;
        Ok(())
    }

    /// Writes to a port mask, which is assumed to be a pair of adjacent
    /// registers representing all 53 ports.
    pub fn write_port_mask<T>(
        &self,
        mut reg: RegisterAddress<T>,
        value: u64,
    ) -> Result<(), VscError>
    where
        T: From<u32>,
        u32: From<T>,
    {
        self.write(reg, ((value & 0xFFFFFFFF) as u32).into())?;
        reg.addr += 4; // Good luck!
        self.write(reg, (((value >> 32) as u32) & 0x1FFFFF).into())
    }

    /// Performs a write operation on the given register, where the value is
    /// calculated by calling f(0).  This is helpful as a way to reduce manual
    /// type information.
    pub fn write_with<T, F>(
        &self,
        reg: RegisterAddress<T>,
        f: F,
    ) -> Result<(), VscError>
    where
        T: From<u32>,
        u32: From<T>,
        F: Fn(&mut T),
    {
        let mut data = 0.into();
        f(&mut data);
        self.write(reg, data)
    }

    /// Performs a read-modify-write operation on a VSC7448 register
    pub fn modify<T, F>(
        &self,
        reg: RegisterAddress<T>,
        f: F,
    ) -> Result<(), VscError>
    where
        T: From<u32>,
        u32: From<T>,
        F: Fn(&mut T),
    {
        let mut data = self.read(reg)?;
        f(&mut data);
        self.write(reg, data)
    }

    /// Builds a MII_CMD register based on the given phy and register.  Note
    /// that miim_cmd_opr_field is unset; you must configure it for a read
    /// or write yourself.
    pub fn miim_cmd(
        phy: u8,
        reg_addr: u8,
    ) -> vsc7448_pac::devcpu_gcb::miim::MII_CMD {
        let mut v: vsc7448_pac::devcpu_gcb::miim::MII_CMD = 0.into();
        v.set_miim_cmd_vld(1);
        v.set_miim_cmd_phyad(phy as u32);
        v.set_miim_cmd_regad(reg_addr as u32);
        v
    }

    /// Writes a register to the PHY without modifying the page.  This
    /// shouldn't be called directly, as the page could be in an unknown
    /// state.
    fn phy_write_inner<T: From<u16>>(
        &self,
        miim: u8,
        phy: u8,
        reg: PhyRegisterAddress<T>,
        value: T,
    ) -> Result<(), VscError>
    where
        u16: From<T>,
    {
        let value: u16 = value.into();
        let mut v = Self::miim_cmd(phy, reg.addr);
        v.set_miim_cmd_opr_field(0b01); // read
        v.set_miim_cmd_wrdata(value as u32);

        self.miim_idle_wait(miim)?;
        self.write(Vsc7448::DEVCPU_GCB().MIIM(miim as u32).MII_CMD(), v)
    }

    /// Waits for the PENDING_RD and PENDING_WR bits to go low, indicating that
    /// it's safe to read or write to the MIIM.
    fn miim_idle_wait(&self, miim: u8) -> Result<(), VscError> {
        for _i in 0..32 {
            let status = self
                .read(Vsc7448::DEVCPU_GCB().MIIM(miim as u32).MII_STATUS())?;
            if status.miim_stat_opr_pend() == 0 {
                return Ok(());
            } else {
                ringbuf_entry!(Trace::MiimIdleWait);
            }
        }
        return Err(VscError::MiimIdleTimeout);
    }

    /// Waits for the STAT_BUSY bit to go low, indicating that a read has
    /// finished and data is available.
    fn miim_read_wait(&self, miim: u8) -> Result<(), VscError> {
        for _i in 0..32 {
            let status = self
                .read(Vsc7448::DEVCPU_GCB().MIIM(miim as u32).MII_STATUS())?;
            if status.miim_stat_busy() == 0 {
                return Ok(());
            } else {
                ringbuf_entry_masked!(DEBUG_TRACE_MIIM, Trace::MiimReadWait);
            }
        }
        return Err(VscError::MiimReadTimeout);
    }

    /// Reads a register from the PHY without modifying the page.  This
    /// shouldn't be called directly, as the page could be in an unknown
    /// state.
    fn phy_read_inner<T: From<u16>>(
        &self,
        miim: u8,
        phy: u8,
        reg: PhyRegisterAddress<T>,
    ) -> Result<T, VscError> {
        let mut v = Self::miim_cmd(phy, reg.addr);
        v.set_miim_cmd_opr_field(0b10); // read

        self.miim_idle_wait(miim)?;
        self.write(Vsc7448::DEVCPU_GCB().MIIM(miim as u32).MII_CMD(), v)?;
        self.miim_read_wait(miim)?;

        let out =
            self.read(Vsc7448::DEVCPU_GCB().MIIM(miim as u32).MII_DATA())?;
        if out.miim_data_success() == 0b11 {
            return Err(VscError::MiimReadErr {
                miim,
                phy,
                page: reg.page,
                addr: reg.addr,
            });
        }

        let value = out.miim_data_rddata() as u16;
        Ok(value.into())
    }

    /// Reads a register from the PHY using the MIIM interface
    pub fn phy_read<T>(
        &self,
        miim: u8,
        phy: u8,
        reg: PhyRegisterAddress<T>,
    ) -> Result<T, VscError>
    where
        T: From<u16> + Clone,
        u16: From<T>,
    {
        ringbuf_entry_masked!(
            DEBUG_TRACE_MIIM,
            Trace::MiimSetPage {
                miim,
                phy,
                page: reg.page,
            }
        );
        self.phy_write_inner::<phy::standard::PAGE>(
            miim,
            phy,
            phy::STANDARD::PAGE(),
            reg.page.into(),
        )?;
        let out = self.phy_read_inner(miim, phy, reg)?;
        ringbuf_entry_masked!(
            DEBUG_TRACE_MIIM,
            Trace::MiimRead {
                miim,
                phy,
                page: reg.page,
                addr: reg.addr,
                value: out.clone().into(),
            }
        );
        Ok(out)
    }

    /// Writes a register to the PHY using the MIIM interface
    pub fn phy_write<T>(
        &self,
        miim: u8,
        phy: u8,
        reg: PhyRegisterAddress<T>,
        value: T,
    ) -> Result<(), VscError>
    where
        T: From<u16> + Clone,
        u16: From<T>,
    {
        ringbuf_entry_masked!(
            DEBUG_TRACE_MIIM,
            Trace::MiimSetPage {
                miim,
                phy,
                page: reg.page,
            }
        );
        self.phy_write_inner::<phy::standard::PAGE>(
            miim,
            phy,
            phy::STANDARD::PAGE(),
            reg.page.into(),
        )?;
        ringbuf_entry_masked!(
            DEBUG_TRACE_MIIM,
            Trace::MiimWrite {
                miim,
                phy,
                page: reg.page,
                addr: reg.addr,
                value: value.clone().into(),
            }
        );
        self.phy_write_inner(miim, phy, reg, value)
    }

    /// Performs a read-modify-write operation on a PHY register connected
    /// to the VSC7448 via MIIM.
    pub fn phy_modify<T, F>(
        &self,
        miim: u8,
        phy: u8,
        reg: PhyRegisterAddress<T>,
        f: F,
    ) -> Result<(), VscError>
    where
        T: From<u16> + Clone,
        u16: From<T>,
        F: Fn(&mut T),
    {
        let mut data = self.phy_read(miim, phy, reg)?;
        f(&mut data);
        self.phy_write(miim, phy, reg, data)
    }
}
