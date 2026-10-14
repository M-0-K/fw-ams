import type { CardKind } from "./bindings/CardKind";
import type { ChargeState } from "./bindings/ChargeState";
import type { Model } from "./bindings/Model";

const de = (digits: number) =>
  new Intl.NumberFormat("de-DE", { minimumFractionDigits: digits, maximumFractionDigits: digits });

export function num(v: number | null | undefined, digits = 1, unit = ""): string {
  if (v === null || v === undefined || Number.isNaN(v)) return "n/v";
  return `${de(digits).format(v)}${unit ? ` ${unit}` : ""}`;
}

export function signed(v: number | null | undefined, digits = 1, unit = ""): string {
  if (v === null || v === undefined) return "n/v";
  const s = num(v, digits, unit);
  return v > 0 ? `+${s}` : s;
}

export function duration(minutes: number | null): string {
  if (minutes === null) return "n/v";
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0 ? `${h} h ${m.toString().padStart(2, "0")} min` : `${m} min`;
}

export const chargeStateLabel: Record<ChargeState, string> = {
  charging: "Lädt",
  discharging: "Entlädt",
  idle: "Am Netz",
  unknown: "Unbekannt",
};

export const modelLabel: Record<Model, string> = {
  laptop12: "Framework Laptop 12",
  laptop13Intel: "Framework Laptop 13 (Intel)",
  laptop13CoreUltra: "Framework Laptop 13 (Intel Core Ultra)",
  laptop13Amd7040: "Framework Laptop 13 (AMD Ryzen 7040)",
  laptop13AmdAi300: "Framework Laptop 13 (AMD Ryzen AI 300)",
  laptop16Amd7040: "Framework Laptop 16 (AMD Ryzen 7040)",
  laptop16AmdAi300: "Framework Laptop 16 (AMD Ryzen AI 300)",
  desktop: "Framework Desktop",
  unknown: "Unbekanntes Gerät",
};

/** Short label and color per card type for the slot diagram */
export const cardStyle: Record<CardKind, { short: string; color: string }> = {
  emptyOrPassive: { short: "leer", color: "bg-slate-300 dark:bg-slate-700 text-slate-600 dark:text-slate-300" },
  usbC: { short: "USB-C", color: "bg-sky-500 text-white" },
  usbA: { short: "USB-A", color: "bg-indigo-500 text-white" },
  hdmi: { short: "HDMI", color: "bg-orange-500 text-white" },
  displayPort: { short: "DP", color: "bg-amber-500 text-white" },
  audio: { short: "Audio", color: "bg-pink-500 text-white" },
  sd: { short: "SD", color: "bg-emerald-500 text-white" },
  microSd: { short: "µSD", color: "bg-emerald-600 text-white" },
  storage: { short: "SSD", color: "bg-teal-600 text-white" },
  ethernet: { short: "LAN", color: "bg-violet-500 text-white" },
  unknown: { short: "?", color: "bg-slate-500 text-white" },
};

export function tempColor(c: number | null): string {
  if (c === null) return "bg-slate-400";
  if (c >= 90) return "bg-red-500";
  if (c >= 75) return "bg-orange-500";
  if (c >= 60) return "bg-amber-400";
  return "bg-emerald-500";
}
