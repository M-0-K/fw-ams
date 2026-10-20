import type { PowerMode } from "../../bindings/PowerMode";
import type { ProfileSlot } from "../../bindings/ProfileSlot";

/** Runs a backend action with busy state and a status message; resolves to success */
export type Run = (what: () => Promise<unknown>, ok: string) => Promise<boolean>;

export const slotLabel: Record<ProfileSlot, string> = { ac: "Netzteil", dc: "Akku", saver: "Energiesparen" };

export const powerModes: { id: PowerMode; label: string; desc: string }[] = [
  { id: "efficiency", label: "Beste Energieeffizienz", desc: "Framework: 15 W / 25 W" },
  { id: "balanced", label: "Ausbalanciert", desc: "Framework: 25 W / 33 W" },
  { id: "performance", label: "Beste Leistung", desc: "Framework: 30 W / 35 W" },
];

export const modeLabel = (m: PowerMode) => powerModes.find((p) => p.id === m)?.label ?? "Anderer";
