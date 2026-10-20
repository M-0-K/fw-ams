import { useEffect, useState } from "react";
import type { Snapshot } from "../bindings/Snapshot";
import type { Slot } from "../bindings/Slot";
import type { PdPort } from "../bindings/PdPort";
import { forgetCards } from "../api";
import LaptopDiagram from "../components/LaptopDiagram";
import { Button, Notice } from "../components/ui";
import { cardStyle, num } from "../format";
import { describe, flow, isSilent, lastSeen } from "../slots";

const confidenceLabel = {
  certain: "sicher erkannt",
  likely: "vermutet",
  remembered: "gemerkt – meldet sich gerade nicht",
  unknown: "unklar",
} as const;

const partnerLabel = {
  nothing: "nichts",
  sink: "Verbraucher",
  source: "Stromquelle",
  debugAccessory: "Debug-Zubehör",
  audioAccessory: "Audio-Zubehör",
  poweredAccessory: "Zubehör (versorgt)",
  unknown: "unbekannt",
} as const;

function yesNo(v: boolean | null) {
  return v === null ? "?" : v ? "ja" : "nein";
}

function Rows({ rows }: { rows: [string, string][] }) {
  return (
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
  );
}

function PortTable({ port: p }: { port: PdPort }) {
  return (
    <div>
      <div className="label mb-1">USB-C PD-Port {p.index}</div>
      <Rows
        rows={[
          ["Gegenstelle", partnerLabel[p.partner]],
          ["Stromrolle", p.powerRole === "sink" ? "Laptop lädt" : p.powerRole === "source" ? "Laptop versorgt" : "—"],
          ["Spannung / Strom", `${num(p.voltageV, 2, "V")} / ${num(p.currentA, 2, "A")}`],
          ["Leistung", num(p.powerW, 1, "W")],
        ]}
      />
    </div>
  );
}

function PortRaw({ port: p }: { port: PdPort }) {
  return (
    <Rows
      rows={[
        ["Datenrolle", p.dataRole === "host" ? "Host (DFP)" : p.dataRole === "device" ? "Gerät (UFP)" : "—"],
        ["PD-Vertrag", p.pdContract ? "ja" : "nein"],
        ["Max. (Partner)", num(p.maxPowerW, 0, "W")],
        ["VCONN", p.vconn ? "an" : "aus"],
        ["EPR", p.eprActive ? "aktiv" : p.eprSupported ? "unterstützt" : "nein"],
        ["DP Alt Mode", p.dpAltMode ? "aktiv" : "nein"],
      ]}
    />
  );
}

function SlotDetail({ slot, port }: { slot: Slot; port?: PdPort }) {
  const c = slot.capabilities;
  const f = flow(port);
  const hasPartner = port && port.partner !== "nothing";
  return (
    <div className="space-y-4">
      <div>
        <div className="label">
          Slot {slot.number} · {slot.location.label}
          {slot.pdPort !== null && ` · PD-Port ${slot.pdPort}`}
        </div>
        <h3 className="text-2xl font-bold">{slot.card.name}</h3>
        <div className="mt-1 flex flex-wrap items-center gap-2 text-sm">
          <span className="font-medium">{describe(slot, port)}</span>
          {f && (
            <span
              className={`rounded-full px-2 py-0.5 text-xs font-semibold tabular-nums ${
                f.dir === "in"
                  ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
                  : "bg-sky-100 text-sky-700 dark:bg-sky-950 dark:text-sky-300"
              }`}
            >
              {f.dir === "in" ? "↓" : "↑"} {f.upTo && "≤ "}
              {num(f.watts, f.watts < 10 ? 1 : 0, "W")}
            </span>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-x-2 text-xs text-slate-500">
          <span>
            {confidenceLabel[slot.card.confidence]}
            {slot.card.usbId && ` · USB ${slot.card.usbId}`}
            {slot.card.rememberedMs !== null && ` · zuletzt sicher erkannt ${lastSeen(slot.card.rememberedMs)}`}
          </span>
        </div>
      </div>

      {slot.hints.length > 0 && (
        <div className="space-y-2">
          {slot.hints.map((h, i) => (
            <Notice key={i} tone={h.severity === "warning" ? "error" : "info"}>
              {h.message}
            </Notice>
          ))}
        </div>
      )}
      {slot.card.rememberedMs !== null && (
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <Button onClick={() => forgetCards(slot.number).catch(() => {})}>Slot vergessen</Button>
          <span className="text-xs text-slate-500">Danach zeigt der Slot nur noch, was sich gerade meldet.</span>
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

      {port &&
        (hasPartner ? (
          <PortTable port={port} />
        ) : (
          <div className="text-sm text-slate-500">
            USB-C PD-Port {port.index}: keine Gegenstelle – kein Strom- oder Datenfluss.
          </div>
        ))}

      {(hasPartner || slot.devices.length > 0) && (
        <details className="text-sm">
          <summary className="label cursor-pointer select-none">Technische Details</summary>
          <div className="mt-2 space-y-3">
            {hasPartner && <PortRaw port={port} />}
            {slot.devices.length > 0 && (
              <div>
                <div className="label mb-1">USB-Geräte</div>
                {slot.devices.map((d) => (
                  <div key={d.id + d.rootPort}>
                    {d.product ?? d.id}{" "}
                    <span className="text-slate-500">
                      ({d.id}, {d.rootPort}, USB {d.usbVersion})
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </details>
      )}
    </div>
  );
}

export default function Ports({ s, initialSlot }: { s: Snapshot; initialSlot?: number }) {
  const [selected, setSelected] = useState<number>(() => {
    if (initialSlot !== undefined) return initialSlot;
    const interesting = s.slots.find((x) => x.card.kind !== "emptyOrPassive");
    return (interesting ?? s.slots[0])?.number ?? 1;
  });
  // Jumping here from the overview or flyout while this page is already open
  useEffect(() => {
    if (initialSlot !== undefined) setSelected(initialSlot);
  }, [initialSlot]);
  const slot = s.slots.find((x) => x.number === selected) ?? s.slots[0];
  const port = s.ports.find((p) => p.index === slot?.pdPort);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[auto_1fr]">
      <section className="card flex flex-col items-center justify-center gap-4">
        <LaptopDiagram slots={s.slots} selected={selected} onSelect={setSelected} />
        <div className="w-full max-w-sm space-y-1">
          {s.slots.map((x) => (
            <button
              key={x.number}
              onClick={() => setSelected(x.number)}
              className={`flex w-full items-center gap-3 rounded-lg px-2 py-1 text-left text-sm transition ${
                x.number === selected ? "bg-accent/10" : "hover:bg-slate-100 dark:hover:bg-slate-800"
              }`}
            >
              <span
                className={`w-12 shrink-0 rounded-md py-0.5 text-center text-[11px] font-bold ${cardStyle[x.card.kind].color} ${
                  isSilent(x) ? "opacity-60" : ""
                }`}
              >
                {cardStyle[x.card.kind].short}
              </span>
              <span className="w-28 shrink-0 text-xs text-slate-500">{x.location.label}</span>
              <span className="truncate">{describe(x, s.ports.find((p) => p.index === x.pdPort))}</span>
            </button>
          ))}
        </div>
        <p className="max-w-xs text-center text-xs text-slate-500">
          „?“ = aus dem Port-Status abgeleitet, „↺“ = gemerkt (meldet sich gerade nicht), roter Punkt = Warnung.
        </p>
      </section>
      <section className="card">{slot ? <SlotDetail slot={slot} port={port} /> : "Keine Slots"}</section>
    </div>
  );
}
