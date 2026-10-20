//! Fuses USB enumeration, PD port state and the slot layout into per-slot card information.
//!
//! Active cards (HDMI, DP, audio, storage, …) are identified by USB ID. Passive cards are
//! inferred from the PD controller: a USB-A card presents Rd and gets 5 V without a PD
//! contract, a charger can only come through a USB-C card, an empty slot shows nothing.

use super::catalog::{self, card_type, kind_by_usb_id};
use super::slots::{usb_a_standby_penalty, SlotDef};
use crate::model::{
    Card, CardKind, Confidence, DataRole, Hint, Location, Model, Partner, PdPort, PowerRole,
    Severity, Slot, SlotCapabilities, UsbDevice,
};

/// Below this a USB-C charger is considered too weak for a laptop
const WEAK_CHARGER_W: f32 = 45.0;

pub fn detect(model: Model, defs: &[SlotDef], ports: &[PdPort], usb: &[UsbDevice]) -> Vec<Slot> {
    // 1. Devices on calibrated root ports
    let mut devices: Vec<Vec<UsbDevice>> = defs
        .iter()
        .map(|def| {
            usb.iter()
                .filter(|d| d.root_port.as_deref().is_some_and(|p| def.usb_ports.contains(&p)))
                .cloned()
                .collect()
        })
        .collect();
    let mut calibrated: Vec<bool> = devices.iter().map(|d| has_card(d)).collect();

    // 2. Cards on uncalibrated ports: place them if exactly one slot looks like an active card
    let known_ports: Vec<&str> = defs.iter().flat_map(|d| d.usb_ports.iter().copied()).collect();
    let unplaced: Vec<&UsbDevice> = usb
        .iter()
        .filter(|d| kind_by_usb_id(d.vid, d.pid).is_some())
        .filter(|d| !d.root_port.as_deref().is_some_and(|p| known_ports.contains(&p)))
        .collect();
    for dev in unplaced {
        let candidates: Vec<usize> = defs
            .iter()
            .enumerate()
            .filter(|(i, def)| {
                !has_card(&devices[*i]) && pd_of(def, ports).is_some_and(looks_like_active_card)
            })
            .map(|(i, _)| i)
            .collect();
        if let [only] = candidates[..] {
            devices[only].push(dev.clone());
            calibrated[only] = false;
        }
    }

    defs.iter()
        .zip(devices)
        .zip(calibrated)
        .map(|((def, devs), calibrated)| build_slot(model, def, pd_of(def, ports), devs, calibrated))
        .collect()
}

fn pd_of<'a>(def: &SlotDef, ports: &'a [PdPort]) -> Option<&'a PdPort> {
    def.pd_port.and_then(|i| ports.iter().find(|p| p.index == i))
}

fn has_card(devs: &[UsbDevice]) -> bool {
    devs.iter().any(|d| kind_by_usb_id(d.vid, d.pid).is_some())
}

/// Active cards are powered by the laptop, negotiate PD and request VCONN
fn looks_like_active_card(p: &PdPort) -> bool {
    p.partner == Partner::Sink && p.pd_contract && p.vconn
}

pub fn card(kind: CardKind, confidence: Confidence, usb_id: Option<String>) -> Card {
    let t = card_type(kind);
    Card {
        kind,
        confidence,
        name: t.name.to_owned(),
        features: t.features.iter().map(|s| s.to_string()).collect(),
        usb_id,
        remembered_ms: None,
    }
}

/// PD port of a slot in the snapshot's port list
pub fn slot_port<'a>(slot: &Slot, ports: &'a [PdPort]) -> Option<&'a PdPort> {
    slot.pd_port.and_then(|i| ports.iter().find(|p| p.index == i))
}

fn classify(pd: Option<&PdPort>, devs: &[UsbDevice], calibrated: bool) -> Card {
    if let Some((dev, kind)) = devs
        .iter()
        .find_map(|d| kind_by_usb_id(d.vid, d.pid).map(|k| (d, k)))
    {
        let conf = if calibrated { Confidence::Certain } else { Confidence::Likely };
        return card(kind, conf, Some(dev.id.clone()));
    }
    let Some(p) = pd else {
        // Slot without PD controller (e.g. Laptop 16 front slots): only USB tells us something
        return if devs.is_empty() {
            card(CardKind::EmptyOrPassive, Confidence::Unknown, None)
        } else {
            card(CardKind::Unknown, Confidence::Unknown, None)
        };
    };
    match p.partner {
        Partner::Nothing => card(CardKind::EmptyOrPassive, Confidence::Likely, None),
        // Only a USB-C card can pass power into the laptop
        Partner::Source => card(CardKind::UsbC, Confidence::Certain, None),
        Partner::AudioAccessory | Partner::DebugAccessory | Partner::PoweredAccessory => {
            card(CardKind::UsbC, Confidence::Likely, None)
        }
        Partner::Sink if p.pd_contract => {
            let mut c = card(CardKind::Unknown, Confidence::Unknown, None);
            c.name = "Aktive Karte oder USB-C-PD-Gerät".into();
            c
        }
        // 5 V without PD contract: the USB-A card's Rd termination
        Partner::Sink => card(CardKind::UsbA, Confidence::Likely, None),
        Partner::Unknown => card(CardKind::Unknown, Confidence::Unknown, None),
    }
}

fn protocols(pd: Option<&PdPort>, devs: &[UsbDevice]) -> Vec<String> {
    let mut out = vec![];
    if let Some(p) = pd.filter(|p| p.partner != Partner::Nothing) {
        let dir = match p.power_role {
            PowerRole::Sink => "Laden",
            PowerRole::Source => "Versorgung",
            PowerRole::None => "Strom",
        };
        if p.pd_contract {
            out.push(format!(
                "USB PD {dir}: {:.1} V × {:.2} A = {:.1} W",
                p.voltage_v, p.current_a, p.power_w
            ));
        } else if p.voltage_v > 0.0 {
            out.push(format!(
                "USB Typ-C {dir} ohne PD: {:.1} V, bis {:.2} A",
                p.voltage_v, p.current_a
            ));
        }
        if p.epr_active {
            out.push("USB PD EPR (> 20 V) aktiv".into());
        }
        if p.dp_alt_mode {
            out.push("DisplayPort Alt Mode aktiv".into());
            if p.alt_mode_raw & 0x80 != 0 {
                out.push("Monitor erkannt (HPD)".into());
            }
        }
        if p.vconn {
            out.push("VCONN aktiv (versorgt aktive Karte/Kabel)".into());
        }
        match p.data_role {
            Some(DataRole::Host) => out.push("Datenrolle: USB-Host (DFP)".into()),
            Some(DataRole::Device) => out.push("Datenrolle: USB-Gerät (UFP)".into()),
            None => {}
        }
    }
    for d in devs {
        let name = d.product.clone().unwrap_or_else(|| d.id.clone());
        out.push(format!("{name}: {}", d.speed.describe()));
    }
    out
}

/// Hints for the card in a slot. Public so the card memory can redo them after replacing the card.
pub fn slot_hints(model: Model, slot: &Slot, pd: Option<&PdPort>, calibrated: bool) -> Vec<Hint> {
    hints(model, &slot.location, &slot.capabilities, pd, &slot.card, calibrated)
}

fn hints(
    model: Model,
    location: &Location,
    capabilities: &SlotCapabilities,
    pd: Option<&PdPort>,
    card: &Card,
    calibrated: bool,
) -> Vec<Hint> {
    let mut out = vec![];
    let mut push = |severity, message: String| out.push(Hint { severity, message });

    if catalog::needs_display(card.kind) {
        if capabilities.display == Some(false) {
            push(
                Severity::Warning,
                format!("{} unterstützt in diesem Slot keine Displayausgabe.", card.name),
            );
        } else if pd.is_some_and(|p| !p.dp_alt_mode) {
            push(Severity::Info, "Kein Monitor aktiv (DP Alt Mode nicht ausgehandelt).".into());
        }
    }
    if card.kind == CardKind::UsbA && usb_a_standby_penalty(model, location) {
        push(
            Severity::Info,
            "USB-A-Karte in einem hinteren Slot erhöht den Standby-Verbrauch – \
             ein vorderer Slot spart Akku."
                .into(),
        );
    }
    if let Some(p) = pd.filter(|p| p.power_role == PowerRole::Sink && p.partner == Partner::Source) {
        if !p.pd_contract {
            push(Severity::Warning, "Ladegerät ohne USB PD – nur 5 V, lädt sehr langsam.".into());
        } else if p.power_w < WEAK_CHARGER_W {
            push(
                Severity::Warning,
                format!("Ladegerät liefert nur {:.0} W – unter Last kann der Akku sinken.", p.power_w),
            );
        }
    }
    if card.confidence == Confidence::Likely && card.usb_id.is_some() && !calibrated {
        push(
            Severity::Info,
            "USB-Port dieses Slots ist nicht kalibriert – Zuordnung über PD-Status abgeleitet.".into(),
        );
    }
    out
}

fn build_slot(
    model: Model,
    def: &SlotDef,
    pd: Option<&PdPort>,
    devices: Vec<UsbDevice>,
    calibrated: bool,
) -> Slot {
    let card = classify(pd, &devices, calibrated);
    Slot {
        number: def.number,
        location: def.location.clone(),
        pd_port: def.pd_port,
        capabilities: def.capabilities.clone(),
        protocols: protocols(pd, &devices),
        hints: hints(model, &def.location, &def.capabilities, pd, &card, calibrated),
        card,
        devices,
    }
}
