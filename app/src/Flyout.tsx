import { invoke } from "@tauri-apps/api/core";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { useSettings, useSnapshot } from "./api";
import type { PdPort } from "./bindings/PdPort";
import type { Slot } from "./bindings/Slot";
import type { Snapshot } from "./bindings/Snapshot";
import { cardStyle, chargeStateLabel, num, signed } from "./format";
import { describe, flow, isSilent, slotRows } from "./slots";
import { useTheme } from "./theme";

const WIDTH = 400;

function Tile({ slot, port }: { slot?: Slot; port?: PdPort }) {
  if (!slot) return <div />;
  const style = cardStyle[slot.card.kind];
  const f = flow(port);
  const warn = slot.hints.find((h) => h.severity === "warning");
  const silent = isSilent(slot);
  const idle = slot.card.kind === "emptyOrPassive" || silent;
  return (
    <button
      onClick={() => invoke("show_main", { slot: slot.number })}
      title={warn?.message ?? `${slot.location.label}: ${slot.card.name}${silent ? " (gemerkt)" : ""}`}
      className={`flex min-w-0 flex-col gap-1 rounded-xl border p-2 text-left transition hover:brightness-95 dark:hover:brightness-110 ${
        warn
          ? "border-red-400 bg-red-50 dark:border-red-700 dark:bg-red-950"
          : "border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-800"
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <span
          className={`rounded-md px-1.5 py-0.5 text-[11px] font-bold ${style.color} ${
            silent ? "opacity-60 outline-1 -outline-offset-2 outline-dashed outline-white/80" : ""
          }`}
        >
          {style.short}
          {silent && " ↺"}
        </span>
        {f && (
          <span
            className={`text-xs font-semibold tabular-nums ${
              f.dir === "in" ? "text-emerald-600 dark:text-emerald-400" : "text-sky-600 dark:text-sky-400"
            }`}
          >
            {f.dir === "in" ? "↓" : "↑"} {f.upTo && "≤ "}
            {num(f.watts, f.watts < 10 ? 1 : 0, "W")}
          </span>
        )}
      </div>
      <div className={`line-clamp-2 min-h-8 text-xs font-medium leading-4 ${idle ? "text-slate-400 dark:text-slate-500" : ""}`}>
        {describe(slot, port)}
        {port?.dpAltMode && slot.card.kind !== "hdmi" && slot.card.kind !== "displayPort" && " · Display"}
      </div>
      {f && port && (
        <div className="text-[10px] tabular-nums text-slate-500">
          {num(port.voltageV, 0, "V")} · {num(port.currentA, 2, "A")}
        </div>
      )}
    </button>
  );
}

function Content({ s }: { s: Snapshot }) {
  const b = s.battery;
  const portOf = (slot?: Slot) => s.ports.find((p) => p.index === slot?.pdPort);
  const watts = (role: "sink" | "source") =>
    s.ports.filter((p) => p.powerRole === role).reduce((sum, p) => sum + p.powerW, 0);
  const totalIn = watts("sink");
  const totalOut = watts("source");
  const hottest = s.thermal?.sensors
    .filter((x) => x.celsius !== null)
    .sort((a, c) => (c.celsius ?? 0) - (a.celsius ?? 0))[0];
  const fan = s.thermal?.fans[0];
  const rows = slotRows(s.slots);

  return (
    <>
      <div className="flex items-center gap-3 border-b border-slate-200 pb-3 dark:border-slate-800">
        <div className="text-3xl font-bold tabular-nums">{b ? `${Math.round(b.socPercent)}%` : "—"}</div>
        <div className="flex-1 text-sm">
          <div className="font-medium">
            {b ? chargeStateLabel[b.state] : "kein Akku"}
            {b?.chargeLimitPercent != null && b.chargeLimitPercent < 100 && (
              <span className="text-slate-500"> · Limit {b.chargeLimitPercent} %</span>
            )}
          </div>
          <div className="text-slate-500 tabular-nums">Akku {signed(b?.powerW ?? null, 1, "W")}</div>
        </div>
        <div className="text-right text-xs tabular-nums">
          <div className="text-emerald-600 dark:text-emerald-400">↓ rein {num(totalIn, 0, "W")}</div>
          <div className="text-sky-600 dark:text-sky-400">↑ raus ≤ {num(totalOut, 1, "W")}</div>
        </div>
      </div>

      {/* Slots as they sit in the chassis: left side, body, right side */}
      <div className="py-3">
        <div className="mb-1 text-center text-[10px] uppercase tracking-widest text-slate-400">hinten</div>
        <div className="grid grid-cols-[1fr_28px_1fr] gap-x-1 gap-y-2">
          {rows.map((row, i) => (
            <div key={row.position} className="contents">
              <Tile slot={row.left} port={portOf(row.left)} />
              <div
                className={`bg-slate-200 dark:bg-slate-700 ${i === 0 ? "rounded-t-xl" : ""} ${
                  i === rows.length - 1 ? "rounded-b-xl" : ""
                }`}
              />
              <Tile slot={row.right} port={portOf(row.right)} />
            </div>
          ))}
        </div>
        <div className="mt-1 text-center text-[10px] uppercase tracking-widest text-slate-400">vorne</div>
      </div>

      <div className="flex items-center justify-between border-t border-slate-200 pt-2 text-xs text-slate-500 dark:border-slate-800">
        <span>
          {hottest ? `${hottest.label} ${num(hottest.celsius, 0, "°C")}` : ""}
          {fan?.rpm != null && ` · Lüfter ${fan.rpm} RPM`}
          {s.thermal?.throttle?.soft || s.thermal?.throttle?.hard ? " · gedrosselt" : ""}
        </span>
        <button onClick={() => invoke("show_main", { slot: null })} className="font-medium text-accent hover:underline">
          App öffnen →
        </button>
      </div>
    </>
  );
}

export default function Flyout() {
  const s = useSnapshot();
  useTheme(useSettings()?.theme);
  const ref = useRef<HTMLDivElement>(null);

  // Fit the window height to the content
  useEffect(() => {
    if (!ref.current) return;
    const observer = new ResizeObserver(([entry]) => {
      const height = Math.ceil(entry.target.getBoundingClientRect().height);
      getCurrentWindow().setSize(new LogicalSize(WIDTH, height)).catch(() => {});
    });
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={ref} className="space-y-1 overflow-hidden bg-white p-4 dark:bg-slate-900">
      {s ? <Content s={s} /> : <div className="py-8 text-center text-sm text-slate-400">Lese Hardware…</div>}
    </div>
  );
}
