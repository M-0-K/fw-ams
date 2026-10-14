import { useState } from "react";
import type { Snapshot } from "../bindings/Snapshot";
import type { Slot } from "../bindings/Slot";
import type { PdPort } from "../bindings/PdPort";
import LaptopDiagram from "../components/LaptopDiagram";
import { num } from "../format";

const confidenceLabel = { certain: "sicher erkannt", likely: "vermutet", unknown: "unklar" } as const;

function yesNo(v: boolean | null) {
  return v === null ? "?" : v ? "ja" : "nein";
}

function SlotDetail({ slot, port }: { slot: Slot; port?: PdPort }) {
  const c = slot.capabilities;
  return (
    <div className="space-y-4">
      <div>
        <div className="label">
          Slot {slot.number} · {slot.location.label}
          {slot.pdPort !== null && ` · PD-Port ${slot.pdPort}`}
        </div>
        <h3 className="text-2xl font-bold">{slot.card.name}</h3>
        <div className="text-sm text-slate-500">
          {confidenceLabel[slot.card.confidence]}
          {slot.card.usbId && ` · USB ${slot.card.usbId}`}
        </div>
      </div>

      {slot.hints.length > 0 && (
        <div className="space-y-2">
          {slot.hints.map((h, i) => (
            <div
              key={i}
              className={`rounded-lg p-3 text-sm ${
                h.severity === "warning"
                  ? "bg-red-50 text-red-800 dark:bg-red-950 dark:text-red-200"
                  : "bg-sky-50 text-sky-800 dark:bg-sky-950 dark:text-sky-200"
              }`}
            >
              {h.message}
            </div>
          ))}
        </div>
      )}

      <div>
        <div className="label mb-1">Aktive Protokolle</div>
        {slot.protocols.length ? (
          <ul className="space-y-1 text-sm">
            {slot.protocols.map((p) => (
              <li key={p} className="flex gap-2">
                <span className="text-accent">●</span>
                {p}
              </li>
            ))}
          </ul>
        ) : (
          <div className="text-sm text-slate-400">nichts aktiv</div>
        )}
      </div>

      {slot.card.features.length > 0 && (
        <div>
          <div className="label mb-1">Was die Karte kann</div>
          <ul className="list-inside list-disc space-y-1 text-sm text-slate-600 dark:text-slate-300">
            {slot.card.features.map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        </div>
      )}

      <div>
        <div className="label mb-1">Was der Slot kann {!c.verified && <span className="normal-case">(nicht verifiziert)</span>}</div>
        <div className="grid grid-cols-4 gap-2 text-center text-sm">
          {[
            ["USB4", yesNo(c.usb4)],
            ["max.", c.maxGbps !== null ? `${c.maxGbps} Gbit/s` : "?"],
            ["Display", yesNo(c.display)],
            ["Laden", yesNo(c.charging)],
          ].map(([k, v]) => (
            <div key={k} className="rounded-lg bg-slate-100 p-2 dark:bg-slate-800">
              <div className="text-xs text-slate-500">{k}</div>
              <div className="font-semibold">{v}</div>
            </div>
          ))}
        </div>
      </div>

      {port && <PortTable port={port} />}

      {slot.devices.length > 0 && (
        <div>
          <div className="label mb-1">USB-Geräte</div>
          {slot.devices.map((d) => (
            <div key={d.id + d.rootPort} className="text-sm">
              {d.product ?? d.id} <span className="text-slate-500">({d.id}, {d.rootPort}, USB {d.usbVersion})</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function PortTable({ port: p }: { port: PdPort }) {
  const partner = {
    nothing: "nichts",
    sink: "Verbraucher",
    source: "Stromquelle",
    debugAccessory: "Debug-Zubehör",
    audioAccessory: "Audio-Zubehör",
    poweredAccessory: "Zubehör (versorgt)",
    unknown: "unbekannt",
  }[p.partner];
  const rows: [string, string][] = [
    ["Gegenstelle", partner],
    ["Stromrolle", p.powerRole === "sink" ? "Laptop lädt" : p.powerRole === "source" ? "Laptop versorgt" : "—"],
    ["Datenrolle", p.dataRole === "host" ? "Host (DFP)" : p.dataRole === "device" ? "Gerät (UFP)" : "—"],
    ["PD-Vertrag", p.pdContract ? "ja" : "nein"],
    ["Spannung / Strom", `${num(p.voltageV, 2, "V")} / ${num(p.currentA, 2, "A")}`],
    ["Leistung", num(p.powerW, 1, "W")],
    ["Max. (Partner)", num(p.maxPowerW, 0, "W")],
    ["VCONN", p.vconn ? "an" : "aus"],
    ["EPR", p.eprActive ? "aktiv" : p.eprSupported ? "unterstützt" : "nein"],
    ["DP Alt Mode", p.dpAltMode ? "aktiv" : "nein"],
  ];
  return (
    <div>
      <div className="label mb-1">USB-C PD-Port {p.index}</div>
      <table className="w-full text-sm">
        <tbody>
          {rows.map(([k, v]) => (
            <tr key={k} className="border-b border-slate-100 last:border-0 dark:border-slate-800">
              <td className="py-1 text-slate-500">{k}</td>
              <td className="py-1 text-right font-medium tabular-nums">{v}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default function Ports({ s }: { s: Snapshot }) {
  const [selected, setSelected] = useState<number>(() => {
    const interesting = s.slots.find((x) => x.card.kind !== "emptyOrPassive");
    return (interesting ?? s.slots[0])?.number ?? 1;
  });
  const slot = s.slots.find((x) => x.number === selected) ?? s.slots[0];
  const port = s.ports.find((p) => p.index === slot?.pdPort);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[auto_1fr]">
      <section className="card flex flex-col items-center justify-center gap-4">
        <LaptopDiagram slots={s.slots} selected={selected} onSelect={setSelected} />
        <p className="max-w-xs text-center text-xs text-slate-500">
          Slot anklicken für Details. „?“ = aus PD-Status abgeleitet, roter Punkt = Warnung.
        </p>
      </section>
      <section className="card">{slot ? <SlotDetail slot={slot} port={port} /> : "Keine Slots"}</section>
    </div>
  );
}
