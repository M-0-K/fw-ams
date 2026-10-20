//! Remembers which card sat in which slot.
//!
//! Passive cards are invisible to the laptop without a device (USB-C card) or once the EC has
//! powered the port down, e.g. after standby (USB-A card). A weak detection then falls back to
//! the last confident one instead of showing an empty slot.
//!
//! Rules per slot:
//! - A certain detection (USB ID on a calibrated port, charger on a USB-C card) is learned.
//! - A likely detection is learned unless the remembered card explains it just as well.
//! - What the remembered card explains:
//!   - empty slot: a USB-A or USB-C card (passive, silent). Active cards always draw power, so
//!     for them an empty slot really is empty.
//!   - 5 V sink without PD (normally read as USB-A): a USB-C card with a simple device
//!   - sink with PD contract: a USB-C card or any active card, but not USB-A (no PD)
//!   - unidentified USB devices in a slot without PD controller: any card
//!
//! The app persists the memory (see [`CardMemory::take_save`]).

use super::detect::{self, slot_port};
use crate::model::{CardKind, Confidence, Hint, Model, PdPort, Severity, Slot, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Persist `last_seen` refreshes at most this often (structural changes are saved right away)
const SAVE_EVERY_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Remembered {
    pub kind: CardKind,
    /// Learned from a certain detection
    pub certain: bool,
    /// Unix ms of the last detection that confirmed this card
    pub last_seen_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CardMemory {
    /// Memory belongs to this model; another model starts from scratch
    model: Option<Model>,
    /// Keyed by slot number
    slots: BTreeMap<u8, Remembered>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    touched: bool,
    #[serde(skip)]
    saved_ms: u64,
}

fn is_real(kind: CardKind) -> bool {
    !matches!(kind, CardKind::EmptyOrPassive | CardKind::Unknown)
}

/// Can a remembered card produce what is observed right now?
fn explains(remembered: CardKind, observed: CardKind, pd: Option<&PdPort>) -> bool {
    match observed {
        CardKind::EmptyOrPassive => matches!(remembered, CardKind::UsbA | CardKind::UsbC),
        CardKind::UsbA => remembered == CardKind::UsbC,
        CardKind::Unknown => match pd {
            Some(p) if p.pd_contract => remembered != CardKind::UsbA,
            _ => true,
        },
        _ => false,
    }
}

impl CardMemory {
    pub fn get(&self, slot: u8) -> Option<&Remembered> {
        self.slots.get(&slot)
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Forget one slot or (with `None`) all of them
    pub fn forget(&mut self, slot: Option<u8>) {
        match slot {
            Some(n) => {
                self.slots.remove(&n);
            }
            None => self.slots.clear(),
        }
        self.dirty = true;
    }

    /// Learn from the snapshot and fill in slots whose card is not reporting
    pub fn apply(&mut self, s: &mut Snapshot) {
        let model = s.system.model;
        if self.model != Some(model) {
            self.slots.clear();
            self.model = Some(model);
            self.dirty = true;
        }
        let now = s.timestamp_ms;
        for slot in &mut s.slots {
            let pd = slot_port(slot, &s.ports);
            self.apply_slot(model, slot, pd, now);
        }
    }

    fn apply_slot(&mut self, model: Model, slot: &mut Slot, pd: Option<&PdPort>, now: u64) {
        let observed = &slot.card;
        let memory = self.slots.get(&slot.number).cloned();
        let explained = memory.as_ref().is_some_and(|m| explains(m.kind, observed.kind, pd));

        if is_real(observed.kind) && !explained {
            let certain = observed.confidence == Confidence::Certain;
            if observed.confidence == Confidence::Certain || observed.confidence == Confidence::Likely {
                match self.slots.get_mut(&slot.number) {
                    Some(m) if m.kind == observed.kind => {
                        m.last_seen_ms = now;
                        if certain && !m.certain {
                            m.certain = true;
                            self.dirty = true;
                        }
                        self.touched = true;
                    }
                    // A likely guess never replaces a card that was identified for sure
                    Some(m) if m.certain && !certain => {}
                    _ => {
                        self.slots.insert(
                            slot.number,
                            Remembered { kind: observed.kind, certain, last_seen_ms: now },
                        );
                        self.dirty = true;
                    }
                }
            }
            return;
        }
        let Some(m) = memory.filter(|_| explained) else { return };

        let silent = observed.kind == CardKind::EmptyOrPassive;
        let mut card = detect::card(
            m.kind,
            if silent { Confidence::Remembered } else { Confidence::Likely },
            None,
        );
        card.remembered_ms = Some(m.last_seen_ms);
        slot.card = card;
        slot.hints = detect::slot_hints(model, slot, pd, true);
        slot.hints.push(Hint {
            severity: Severity::Info,
            message: if silent {
                "Gemerkte Karte: meldet sich gerade nicht. Passive USB-A- und USB-C-Karten sind ohne \
                 angeschlossenes Gerät (oder nach dem Standby) für den Laptop unsichtbar."
                    .into()
            } else {
                "Zuordnung über die gemerkte Karte: Der Port-Status allein passt zu mehreren Kartentypen. \
                 Karte getauscht? Dann den Slot vergessen."
                    .into()
            },
        });
    }

    /// True when the memory should be written to disk now; marks it as saved.
    pub fn take_save(&mut self, now_ms: u64) -> bool {
        let due = self.dirty || (self.touched && now_ms.saturating_sub(self.saved_ms) >= SAVE_EVERY_MS);
        if due {
            self.dirty = false;
            self.touched = false;
            self.saved_ms = now_ms;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use crate::model::{Partner, Position, Side};

    fn slot_at(s: &mut Snapshot, side: Side, pos: Position) -> &mut Slot {
        s.slots
            .iter_mut()
            .find(|sl| sl.location.side == side && sl.location.position == pos)
            .unwrap()
    }

    /// Mock snapshot where the USB-A card (left front) and the charger (left back) went silent
    fn silent() -> Snapshot {
        let mut s = mock::snapshot();
        s.timestamp_ms += 60_000;
        for p in &mut s.ports {
            if p.index == 2 || p.index == 3 {
                p.partner = Partner::Nothing;
                p.pd_contract = false;
            }
        }
        for pos in [Position::Front, Position::Back] {
            let sl = slot_at(&mut s, Side::Left, pos);
            sl.card = detect::card(CardKind::EmptyOrPassive, Confidence::Likely, None);
            sl.hints.clear();
        }
        s
    }

    #[test]
    fn silent_passive_cards_are_remembered() {
        let mut m = CardMemory::default();
        let mut s = mock::snapshot();
        let seen = s.timestamp_ms;
        m.apply(&mut s);
        assert!(m.take_save(seen));

        let mut s = silent();
        m.apply(&mut s);
        let lf = slot_at(&mut s, Side::Left, Position::Front);
        assert_eq!(lf.card.kind, CardKind::UsbA);
        assert_eq!(lf.card.confidence, Confidence::Remembered);
        assert_eq!(lf.card.remembered_ms, Some(seen));
        let lb = slot_at(&mut s, Side::Left, Position::Back);
        assert_eq!(lb.card.kind, CardKind::UsbC);
        // The really empty slot stays empty
        let rb = slot_at(&mut s, Side::Right, Position::Back);
        assert_eq!(rb.card.kind, CardKind::EmptyOrPassive);
    }

    #[test]
    fn usb_c_card_with_simple_device_is_not_read_as_usb_a() {
        let mut m = CardMemory::default();
        let mut s = mock::snapshot();
        m.apply(&mut s);
        // Charger replaced by a 5 V device without PD: the PD heuristic says USB-A
        let mut s = mock::snapshot();
        let lb = slot_at(&mut s, Side::Left, Position::Back);
        lb.card = detect::card(CardKind::UsbA, Confidence::Likely, None);
        m.apply(&mut s);
        let lb = slot_at(&mut s, Side::Left, Position::Back);
        assert_eq!(lb.card.kind, CardKind::UsbC);
        assert_eq!(lb.card.confidence, Confidence::Likely);
        assert_eq!(m.get(1).unwrap().kind, CardKind::UsbC);
    }

    #[test]
    fn certain_detection_replaces_memory_and_forget_clears() {
        let mut m = CardMemory::default();
        let mut s = mock::snapshot();
        m.apply(&mut s);
        assert_eq!(m.get(2).unwrap().kind, CardKind::UsbA);

        let mut s = mock::snapshot();
        slot_at(&mut s, Side::Left, Position::Front).card =
            detect::card(CardKind::Sd, Confidence::Certain, Some("32ac:0009".into()));
        m.apply(&mut s);
        assert_eq!(m.get(2).unwrap().kind, CardKind::Sd);

        // An active card that went silent is not invented
        let mut s = silent();
        m.apply(&mut s);
        assert_eq!(slot_at(&mut s, Side::Left, Position::Front).card.kind, CardKind::EmptyOrPassive);

        m.forget(None);
        assert!(m.is_empty());
        assert!(m.take_save(0));
    }

    #[test]
    fn other_model_resets_memory() {
        let mut m = CardMemory::default();
        let mut s = mock::snapshot();
        m.apply(&mut s);
        assert!(!m.is_empty());
        let mut s = silent();
        s.system.model = Model::Laptop13Intel;
        m.apply(&mut s);
        assert_eq!(slot_at(&mut s, Side::Left, Position::Front).card.kind, CardKind::EmptyOrPassive);
    }

    #[test]
    fn roundtrips_as_json() {
        let mut m = CardMemory::default();
        m.apply(&mut mock::snapshot());
        let json = serde_json::to_string(&m).unwrap();
        let back: CardMemory = serde_json::from_str(&json).unwrap();
        assert_eq!(back.get(2), m.get(2));
    }
}
