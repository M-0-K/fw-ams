//! Known Framework expansion cards: USB IDs, names and what they do.

use crate::model::CardKind;

pub struct CardType {
    pub kind: CardKind,
    pub name: &'static str,
    pub features: &'static [&'static str],
}

/// Sources: framework_lib (ccgx/hid.rs, audio_card.rs), Gentoo/Arch wiki "Framework Expansion Cards"
const BY_USB_ID: &[(u16, u16, CardKind)] = &[
    (0x32ac, 0x0002, CardKind::Hdmi),
    (0x32ac, 0x0003, CardKind::DisplayPort),
    (0x32ac, 0x0010, CardKind::Audio),
    (0x32ac, 0x0009, CardKind::Sd),
    (0x090c, 0x3350, CardKind::MicroSd),
    (0x13fe, 0x6500, CardKind::Storage),
    (0x0bda, 0x8156, CardKind::Ethernet),
];

pub fn kind_by_usb_id(vid: u16, pid: u16) -> Option<CardKind> {
    BY_USB_ID
        .iter()
        .find(|(v, p, _)| *v == vid && *p == pid)
        .map(|(_, _, k)| *k)
}

pub fn card_type(kind: CardKind) -> CardType {
    let (name, features): (&str, &[&str]) = match kind {
        CardKind::EmptyOrPassive => (
            "Leer / passive Karte",
            &["Kein Gerät erkannt – Slot leer oder USB-C-Karte ohne angeschlossenes Gerät"],
        ),
        CardKind::UsbC => (
            "USB-C",
            &[
                "Passive Durchleitung des Slot-USB-C-Ports",
                "USB-PD Laden (Sink/Source)",
                "DisplayPort Alt Mode (wenn der Slot es unterstützt)",
                "USB4/Thunderbolt in USB4-fähigen Slots",
            ],
        ),
        CardKind::UsbA => (
            "USB-A",
            &[
                "USB 3.2 Gen 2 (10 Gbit/s) Typ-A Buchse",
                "5 V Versorgung, kein USB-PD",
                "Aktiver Redriver – erhöht Standby-Verbrauch",
            ],
        ),
        CardKind::Hdmi => (
            "HDMI",
            &[
                "HDMI 2.0b, bis 4K60",
                "Nutzt DisplayPort Alt Mode (DP→HDMI-Wandler)",
                "Billboard-Gerät über USB 2.0 (Firmware-Info/Update)",
            ],
        ),
        CardKind::DisplayPort => (
            "DisplayPort",
            &[
                "DisplayPort 1.4, bis 8K60 / 4K120 (je nach Slot/GPU)",
                "Nutzt DisplayPort Alt Mode",
                "Billboard-Gerät über USB 2.0 (Firmware-Info/Update)",
            ],
        ),
        CardKind::Audio => (
            "Audio",
            &[
                "3,5-mm-Klinke, USB Audio Class",
                "Hi-Res Audio DAC/ADC",
            ],
        ),
        CardKind::Sd => ("SD", &["SD-Kartenleser, UHS-I", "USB 3.2 Gen 1 Massenspeicher"]),
        CardKind::MicroSd => (
            "microSD",
            &["microSD-Kartenleser, UHS-I", "USB 3.2 Gen 1 Massenspeicher"],
        ),
        CardKind::Storage => (
            "Speicher (SSD)",
            &["USB 3.2 Gen 2 Massenspeicher (UAS)", "250 GB / 1 TB"],
        ),
        CardKind::Ethernet => (
            "Ethernet 2,5 Gbit",
            &["Realtek RTL8156, 2.5GBASE-T", "USB 3.2 Gen 1, Treiber r8152"],
        ),
        CardKind::Unknown => ("Unbekannt", &[]),
    };
    CardType {
        kind,
        name,
        features,
    }
}

/// Cards that need a display-capable slot
pub fn needs_display(kind: CardKind) -> bool {
    matches!(kind, CardKind::Hdmi | CardKind::DisplayPort)
}
