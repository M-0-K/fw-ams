import type { HistorySample } from "../bindings/HistorySample";
import { num } from "../format";

type Segment = { label: string; value: number; color: string; note: string };

/**
 * Where the power goes right now: one stacked bar, segments with 2px gaps, every segment
 * labeled in the legend (never color alone). Only shows what is actually measured.
 */
export default function PowerBreakdown({ sample }: { sample: HistorySample | null }) {
  if (!sample) return <div className="text-sm text-slate-400">Sammle Daten…</div>;
  const { systemW, apuW, coresW, chargeW } = sample;

  const segments: Segment[] = [];
  if (apuW !== null) {
    if (coresW !== null && coresW <= apuW) {
      segments.push({ label: "CPU-Kerne", value: coresW, color: "var(--series-1)", note: "Summe der Kernleistungen (SMU)" });
      segments.push({
        label: "Grafik, SoC, Speichercontroller",
        value: apuW - coresW,
        color: "var(--series-2)",
        note: "APU-Leistung minus Kerne",
      });
    } else {
      segments.push({ label: "APU (CPU + Grafik + SoC)", value: apuW, color: "var(--series-1)", note: "PPT-Fast-Ist-Wert der SMU" });
    }
  }
  if (systemW !== null) {
    const rest = systemW - (apuW ?? 0);
    if (rest > 0) {
      segments.push({
        label: apuW !== null ? "Rest: Display, RAM, SSD, WLAN, Wandler" : "Gesamtsystem",
        value: rest,
        color: "var(--series-3)",
        note: apuW !== null ? "Akku-Entladeleistung minus APU" : "Akku-Entladeleistung",
      });
    }
  }
  const total = systemW ?? segments.reduce((a, s) => a + s.value, 0);

  return (
    <div>
      <div className="mb-3 flex items-baseline gap-3">
        <span className="text-3xl font-semibold tabular-nums">{num(total, 1, "W")}</span>
        <span className="text-sm text-slate-500">
          {systemW !== null ? "Gesamtsystem (Akku-Entladung)" : "APU – Gesamtsystem nur im Akkubetrieb messbar"}
        </span>
      </div>
      <div className="flex h-6 w-full gap-0.5 overflow-hidden rounded-md bg-slate-100 dark:bg-slate-800" role="img" aria-label="Leistungsaufteilung">
        {segments.map((s) => (
          <div
            key={s.label}
            className="h-full first:rounded-l-md last:rounded-r-md"
            style={{ width: `${total > 0 ? (s.value / total) * 100 : 0}%`, background: s.color, transition: "width 0.6s" }}
            title={`${s.label}: ${num(s.value, 1, "W")}`}
          />
        ))}
      </div>
      <ul className="mt-3 space-y-1.5 text-sm">
        {segments.map((s) => (
          <li key={s.label} className="flex items-center justify-between gap-3">
            <span className="flex min-w-0 items-center gap-2">
              <span className="inline-block h-3 w-3 shrink-0 rounded-sm" style={{ background: s.color }} />
              <span className="truncate">{s.label}</span>
              <span className="hidden text-xs text-slate-400 sm:inline">{s.note}</span>
            </span>
            <span className="shrink-0 tabular-nums">
              <span className="font-semibold">{num(s.value, 1, "W")}</span>
              {total > 0 && <span className="ml-2 text-xs text-slate-500">{num((s.value / total) * 100, 0, "%")}</span>}
            </span>
          </li>
        ))}
        {chargeW !== null && (
          <li className="flex justify-between text-slate-500">
            <span>Ladeleistung in den Akku (zusätzlich vom Netzteil)</span>
            <span className="tabular-nums">{num(chargeW, 1, "W")}</span>
          </li>
        )}
        {segments.length === 0 && <li className="text-slate-400">Keine Leistungsdaten verfügbar.</li>}
      </ul>
    </div>
  );
}
