extern crate alloc;

use core::result;

use crate::info;
use crate::result::Result;
use crate::usb::*;
use crate::xhci::CommandRing;
use crate::xhci::Controller;
use alloc::rc::Rc;
use alloc::vec::Vec;

pub async fn start_usb_tablet(
    _xhc: &Rc<Controller>,
    _slot: u8,
    _ctrl_ep_ring: &mut CommandRing,
    device_descriptor: &UsbDeviceDescriptor,
    descriptors: &Vec<UsbDescriptor>,
) -> Result<()> {
    // vid:pid = 0x0627:0x0001
    // Todo なんでこれでタブレットではないと判定できるのかを調べる
    if device_descriptor.device_class != 0
        || device_descriptor.device_subclass != 0
        || device_descriptor.device_protocol != 0
        || device_descriptor.vender_id != 0x0627
        || device_descriptor.product_id != 0x0001
    {
        return Err("Not a USB Tablet");
    }
    let (_config_desc, _interface_desc_, _) = pick_interface_with_triple(descriptors, (3, 0, 0))
        .ok_or("NO USB KBD Boot interface found")?;
    info!("USB tablet found");
    Ok(())
}
