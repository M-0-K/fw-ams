import { invoke } from "@tauri-apps/api/core";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { useSnapshot } from "./api";
import type { PdPort } from "./bindings/PdPort";
import type { Slot } from "./bindings/Slot";
import type { Snapshot } from "./bindings/Snapshot";
import { cardStyle, chargeStateLabel, num, signed } from "./format";

const WIDTH = 380;

type Flow = { dir: "in" | "out"; watts: number; upTo: boolean };

/** What is attached to the slot, in plain words */
function describe(slot: Slot, port?: PdPort): string {
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

function flow(port?: PdPort): Flow | null {
  if (!port || port.powerW <= 0) return null;
  if (port.powerRole === "sink") return { dir: "in", watts: port.powerW, upTo: false };
  if (port.powerRole === "source") return { dir: "out", watts: port.powerW, upTo: !port.pdContract };
  return null;
}

function SlotRow({ slot, port }: { slot: Slot; port?: PdPort }) {
  const style = cardStyle[slot.card.kind];
  const f = flow(port);
  const warn = slot.hints.find((h) => h.severity === "warning");
  const idle = slot.card.kind === "emptyOrPassive";
  return (
    <div className="flex items-center gap-3 py-2">
      <div className={`flex h-9 w-12 shrink-0 items-center justify-center rounded-lg text-xs font-bold ${style.color}`}>
        {style.short}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[11px] uppercase tracking-wide text-slate-500">{slot.location.label}</div>
        <div className={`truncate text-sm font-medium ${idle ? "text-slate-400" : ""}`}>
          {describe(slot, port)}
          {port?.dpAltMode && slot.card.kind !== "hdmi" && slot.card.kind !== "displayPort" && " · Display"}
        </div>
        {warn && <div className="truncate text-[11px] text-red-500">{warn.message}</div>}
      </div>
      {f && (
        <div className={`shrink-0 text-right tabular-nums ${f.dir === "in" ? "text-emerald-600 dark:text-emerald-400" : "text-sky-600 dark:text-sky-400"}`}>
          <div className="text-sm font-semibold">
            {f.dir === "in" ? "↓" : "↑"} {f.upTo && "≤ "}
            {num(f.watts, f.watts < 10 ? 1 : 0, "W")}
          </div>
          <div className="text-[10px] text-slate-500">
            {num(port!.voltageV, 0, "V")} · {num(port!.currentA, 2, "A")}
          </div>
        </div>
      )}
    </div>
  );
}

function Content({ s }: { s: Snapshot }) {
  const b = s.battery;
  const order = { left: 0, right: 1 } as const;
  const pos = { back: 0, middle: 1, front: 2 } as const;
  const slots = [...s.slots].sort(
    (a, c) => order[a.location.side] - order[c.location.side] || pos[a.location.position] - pos[c.location.position],
  );
  const watts = (role: "sink" | "source") =>
    s.ports.filter((p) => p.powerRole === role).reduce((sum, p) => sum + p.powerW, 0);
  const totalIn = watts("sink");
  const totalOut = watts("source");
  const hottest = s.thermal?.sensors
    .filter((x) => x.celsius !== null)
    .sort((a, c) => (c.celsius ?? 0) - (a.celsius ?? 0))[0];
  const fan = s.thermal?.fans[0];

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

      <div className="divide-y divide-slate-100 dark:divide-slate-800">
        {slots.map((slot) => (
          <SlotRow key={slot.number} slot={slot} port={s.ports.find((p) => p.index === slot.pdPort)} />
        ))}
      </div>

      <div className="flex items-center justify-between border-t border-slate-200 pt-2 text-xs text-slate-500 dark:border-slate-800">
        <span>
          {hottest ? `${hottest.label} ${num(hottest.celsius, 0, "°C")}` : ""}
          {fan?.rpm != null && ` · Lüfter ${fan.rpm} RPM`}
          {s.thermal?.throttle?.soft || s.thermal?.throttle?.hard ? " · gedrosselt" : ""}
        </span>
        <button onClick={() => invoke("show_main")} className="font-medium text-accent hover:underline">
          App öffnen →
        </button>
      </div>
    </>
  );
}

export default function Flyout() {
  const s = useSnapshot();
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
