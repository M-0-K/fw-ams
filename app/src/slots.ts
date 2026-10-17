import type { PdPort } from "./bindings/PdPort";
import type { Slot } from "./bindings/Slot";

export type Flow = { dir: "in" | "out"; watts: number; upTo: boolean };

/** What is attached to the slot, in plain words */
export function describe(slot: Slot, port?: PdPort): string {
  const devices = slot.devices
    .filter((d) => d.id !== slot.card.usbId)
    .map((d) => d.product ?? d.id);
  switch (slot.card.kind) {
    case "hdmi":
    case "displayPort":
      return port?.dpAltMode ? "Monitor angeschlossen" : "kein Monitor";
    case "usbC":
      if (port?.partner === "source") return port.pdContract ? "Ladegerät (USB PD)" : "Ladegerät (5 V, ohne PD)";
      if (port?.partner === "sink") return devices.join(", ") || (port.pdContract ? "Gerät mit USB PD" : "Gerät");
      if (port?.partner === "audioAccessory") return "Audio-Adapter";
      return devices.join(", ") || "Zubehör";
    case "usbA":
      return devices.join(", ") || "kein Gerät erkannt";
    case "emptyOrPassive":
      return "nichts angeschlossen";
    case "unknown":
      return devices.join(", ") || slot.card.name;
    default:
      return devices.join(", ") || "bereit";
  }
}

/** Power flowing through a PD port, from the laptop's point of view */
export function flow(port?: PdPort): Flow | null {
  if (!port || port.powerW <= 0) return null;
  if (port.powerRole === "sink") return { dir: "in", watts: port.powerW, upTo: false };
  if (port.powerRole === "source") return { dir: "out", watts: port.powerW, upTo: !port.pdContract };
  return null;
}

export type SlotRow = { position: "back" | "middle" | "front"; left?: Slot; right?: Slot };

const POSITIONS = ["back", "middle", "front"] as const;

/** Slots grouped into rows from back to front, as they sit in the chassis (2x2, 2x3, 1x2 ...) */
export function slotRows(slots: Slot[]): SlotRow[] {
  return POSITIONS.map((position) => ({
    position,
    left: slots.find((s) => s.location.side === "left" && s.location.position === position),
    right: slots.find((s) => s.location.side === "right" && s.location.position === position),
  })).filter((r) => r.left || r.right);
}
