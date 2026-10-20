//! Physical slot layout per model: position, PD port, capabilities and calibrated USB root ports.
//!
//! PD port index (from framework_lib): 0 right back, 1 right front/middle, 2 left front/middle,
//! 3 left back. Slot numbers follow Framework's knowledge base diagrams.

use crate::model::{Location, Model, Position, Side, SlotCapabilities};

#[derive(Debug, Clone)]
pub struct SlotDef {
    pub number: u8,
    pub location: Location,
    pub pd_port: Option<u8>,
    pub capabilities: SlotCapabilities,
    /// ACPI root ports ("XHC0.PRT2") verified to belong to this slot (USB2 and/or USB3 lane)
    pub usb_ports: &'static [&'static str],
}

fn caps(
    usb4: Option<bool>,
    max_gbps: Option<u32>,
    display: Option<bool>,
    charging: Option<bool>,
    verified: bool,
) -> SlotCapabilities {
    SlotCapabilities {
        usb4,
        max_gbps,
        display,
        charging,
        verified,
    }
}

fn slot(
    number: u8,
    side: Side,
    position: Position,
    pd_port: Option<u8>,
    capabilities: SlotCapabilities,
    usb_ports: &'static [&'static str],
) -> SlotDef {
    SlotDef {
        number,
        location: Location::new(side, position),
        pd_port,
        capabilities,
        usb_ports,
    }
}

/// Four slot layout of Laptop 13 / 12: 1 far left, 2 near left, 3 far right, 4 near right.
fn four_slots(
    back: SlotCapabilities,
    front_left: SlotCapabilities,
    front_right: SlotCapabilities,
    ports: [&'static [&'static str]; 4],
) -> Vec<SlotDef> {
    vec![
        slot(1, Side::Left, Position::Back, Some(3), back.clone(), ports[0]),
        slot(2, Side::Left, Position::Front, Some(2), front_left, ports[1]),
        slot(3, Side::Right, Position::Back, Some(0), back, ports[2]),
        slot(4, Side::Right, Position::Front, Some(1), front_right, ports[3]),
    ]
}

pub fn layout(model: Model) -> Vec<SlotDef> {
    let unknown = caps(None, None, None, None, false);
    match model {
        // https://frame.work/laptop13 (Linux tab): power + display on all four, USB4 on the rear two
        Model::Laptop13AmdAi300 => {
            let rear = caps(Some(true), Some(40), Some(true), Some(true), true);
            let front = caps(Some(false), Some(10), Some(true), Some(true), true);
            // Calibrated on a FW13 AI 300 (BIOS 03.05): HDMI card USB2 lane right front = XHC0.PRT2
            four_slots(rear, front.clone(), front, [&[], &[], &[], &["XHC0.PRT2"]])
        }
        Model::Laptop13Amd7040 => {
            let rear = caps(Some(true), Some(40), Some(true), Some(true), false);
            let front_left = caps(Some(false), Some(10), Some(true), Some(true), false);
            let front_right = caps(Some(false), Some(10), Some(false), Some(true), false);
            four_slots(rear, front_left, front_right, [&[], &[], &[], &[]])
        }
        Model::Laptop13Intel => {
            let all = caps(Some(true), Some(40), Some(true), Some(true), false);
            four_slots(all.clone(), all.clone(), all, [&[], &[], &[], &[]])
        }
        Model::Laptop13CoreUltra | Model::Laptop12 | Model::Unknown => {
            let generic = caps(None, None, Some(true), Some(true), false);
            four_slots(generic.clone(), generic.clone(), generic, [&[], &[], &[], &[]])
        }
        Model::Laptop16Amd7040 | Model::Laptop16AmdAi300 => {
            let rear = caps(Some(true), Some(40), Some(true), Some(true), false);
            let middle = caps(Some(false), Some(10), Some(true), Some(true), false);
            let front = caps(Some(false), Some(10), Some(false), Some(false), false);
            vec![
                slot(1, Side::Left, Position::Back, Some(3), rear.clone(), &[]),
                slot(2, Side::Left, Position::Middle, Some(2), middle.clone(), &[]),
                slot(3, Side::Left, Position::Front, None, front.clone(), &[]),
                slot(4, Side::Right, Position::Back, Some(0), rear, &[]),
                slot(5, Side::Right, Position::Middle, Some(1), middle, &[]),
                slot(6, Side::Right, Position::Front, None, front, &[]),
            ]
        }
        Model::Desktop => vec![
            slot(1, Side::Left, Position::Front, None, unknown.clone(), &[]),
            slot(2, Side::Right, Position::Front, None, unknown, &[]),
        ],
    }
}

/// Location of a PD port independent of slot layout
pub fn pd_port_location(model: Model, port: u8) -> Location {
    let fl16 = matches!(model, Model::Laptop16Amd7040 | Model::Laptop16AmdAi300);
    let middle_or_front = if fl16 { Position::Middle } else { Position::Front };
    match port {
        0 => Location::new(Side::Right, Position::Back),
        1 => Location::new(Side::Right, middle_or_front),
        2 => Location::new(Side::Left, middle_or_front),
        _ => Location::new(Side::Left, Position::Back),
    }
}

/// AMD Laptop 13 "far" (rear) slots draw more standby power with a USB-A card (Framework KB:
/// ~430 mW retimer power vs. 35–120 mW in the front slots)
pub fn usb_a_standby_penalty(model: Model, location: &Location) -> bool {
    matches!(model, Model::Laptop13AmdAi300 | Model::Laptop13Amd7040)
        && location.position == Position::Back
}
