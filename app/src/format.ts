import type { CardKind } from "./bindings/CardKind";
import type { ChargeState } from "./bindings/ChargeState";
import type { Model } from "./bindings/Model";
import type { ParamId } from "./bindings/ParamId";
import type { ParamStatus } from "./bindings/ParamStatus";

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

// --- Advanced / tuner ---------------------------------------------------------------------------


export const paramMeta: Record<ParamId, { label: string; unit: string; digits: number; desc: string }> = {
  stapmW: {
    label: "Dauerleistung (STAPM)",
    unit: "W",
    digits: 1,
    desc: "Leistung, die die APU im Mittel über Minuten halten darf. Bestimmt Lautstärke und Akkulaufzeit.",
  },
  fastW: {
    label: "Kurzboost (PPT Fast)",
    unit: "W",
    digits: 1,
    desc: "Spitzenleistung für wenige Sekunden, z. B. beim Öffnen von Programmen.",
  },
  slowW: {
    label: "Boost (PPT Slow)",
    unit: "W",
    digits: 1,
    desc: "Leistung über einige zehn Sekunden, bevor STAPM greift.",
  },
  tctlC: {
    label: "Temperaturgrenze (Tctl)",
    unit: "°C",
    digits: 0,
    desc: "Ab dieser CPU-Temperatur drosselt die APU. Niedriger = kühler und leiser, aber langsamer.",
  },
  coAll: {
    label: "Curve Optimizer (alle Kerne)",
    unit: "",
    digits: 0,
    desc: "Negativ = weniger Spannung bei gleichem Takt (Undervolting). Zu viel führt zu Abstürzen.",
  },
  maxFreqMhz: {
    label: "Max. CPU-Takt",
    unit: "MHz",
    digits: 0,
    desc: "Windows-Energieplan. 0 = unbegrenzt.",
  },
  boost: {
    label: "Boost-Modus",
    unit: "",
    digits: 0,
    desc: "Windows-Energieplan: 0 = aus, 1 = an, 2 = aggressiv, 3 = effizient, 4 = effizient aggressiv.",
  },
  epp: {
    label: "Energie-Präferenz (EPP)",
    unit: "%",
    digits: 0,
    desc: "Windows-Energieplan: 0 = maximale Leistung, 100 = maximale Effizienz.",
  },
};

export const boostLabels = [
  "Aus",
  "An",
  "Aggressiv",
  "Effizient an",
  "Effizient aggressiv",
  "Aggressiv (garantiert)",
  "Effizient aggressiv (garantiert)",
];

export function paramValue(id: ParamId, v: number | null | undefined): string {
  if (v === null || v === undefined) return "n/v";
  if (id === "boost") return boostLabels[Math.round(v)] ?? num(v, 0);
  if (id === "maxFreqMhz" && v === 0) return "unbegrenzt";
  if (id === "coAll") return signed(v, 0);
  const m = paramMeta[id];
  return num(v, m.digits, m.unit);
}

/** Badge per status: label, colors (with dark variant) and a symbol so it never relies on color alone */
export const statusStyle: Record<ParamStatus, { label: string; icon: string; color: string }> = {
  default: { label: "Standard", icon: "○", color: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300" },
  pending: { label: "ausstehend", icon: "…", color: "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300" },
  applied: { label: "übernommen", icon: "✓", color: "bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-300" },
  unverified: { label: "gesendet, nicht rücklesbar", icon: "↗", color: "bg-sky-100 text-sky-800 dark:bg-sky-950 dark:text-sky-300" },
  limited: { label: "begrenzt", icon: "⊘", color: "bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-300" },
  overridden: { label: "überschrieben", icon: "↺", color: "bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-300" },
  rejected: { label: "abgelehnt", icon: "✕", color: "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300" },
  unsupported: { label: "nicht verfügbar", icon: "–", color: "bg-slate-100 text-slate-400 dark:bg-slate-800 dark:text-slate-500" },
};
