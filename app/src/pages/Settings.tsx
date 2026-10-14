import { useEffect, useState } from "react";
import type { Settings } from "../bindings/Settings";
import type { AlertSettings } from "../bindings/AlertSettings";
import { getSettings, setSettings } from "../api";

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-4 py-2">
      <span className="text-sm">{label}</span>
      <input type="checkbox" className="h-4 w-4 accent-orange-500" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}

function NumberField({
  label,
  value,
  min,
  max,
  unit,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  unit: string;
  onChange: (v: number) => void;
}) {
  return (
    <label className="flex items-center justify-between gap-4 py-2 pl-6 text-sm text-slate-600 dark:text-slate-300">
      <span>{label}</span>
      <span className="flex items-center gap-2">
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          className="w-20 rounded-md border border-slate-300 bg-transparent px-2 py-1 text-right dark:border-slate-700"
        />
        {unit}
      </span>
    </label>
  );
}

export default function SettingsPage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    getSettings().then(setLocal).catch((e) => setStatus(String(e)));
  }, []);

  if (!settings) return <div className="card text-slate-400">{status || "Lade Einstellungen…"}</div>;

  const update = (patch: Partial<Settings>) => setLocal({ ...settings, ...patch });
  const alerts = (patch: Partial<AlertSettings>) => update({ alerts: { ...settings.alerts, ...patch } });
  const a = settings.alerts;

  const save = async () => {
    try {
      setLocal(await setSettings(settings));
      setStatus("Gespeichert");
    } catch (e) {
      setStatus(`Fehler: ${e}`);
    }
    setTimeout(() => setStatus(""), 2500);
  };

  return (
    <div className="max-w-2xl space-y-4">
      <section className="card divide-y divide-slate-100 dark:divide-slate-800">
        <h2 className="pb-2 font-semibold">Allgemein</h2>
        <Toggle label="Mit dem System starten" checked={settings.autostart} onChange={(v) => update({ autostart: v })} />
        <Toggle
          label="Beim Autostart nur im Tray starten"
          checked={settings.startMinimized}
          onChange={(v) => update({ startMinimized: v })}
        />
        <NumberField
          label="Aktualisierungsintervall"
          value={settings.pollIntervalMs / 1000}
          min={0.5}
          max={60}
          unit="s"
          onChange={(v) => update({ pollIntervalMs: Math.round(v * 1000) })}
        />
      </section>

      <section className="card divide-y divide-slate-100 dark:divide-slate-800">
        <h2 className="pb-2 font-semibold">Benachrichtigungen</h2>
        <Toggle label="Akku fast leer" checked={a.batteryLow} onChange={(v) => alerts({ batteryLow: v })} />
        {a.batteryLow && (
          <NumberField label="Schwelle" value={a.batteryLowPercent} min={1} max={99} unit="%" onChange={(v) => alerts({ batteryLowPercent: v })} />
        )}
        <Toggle label="Ladelimit erreicht" checked={a.chargeLimitReached} onChange={(v) => alerts({ chargeLimitReached: v })} />
        <Toggle label="Schwaches Ladegerät" checked={a.weakCharger} onChange={(v) => alerts({ weakCharger: v })} />
        {a.weakCharger && (
          <NumberField label="unter" value={a.weakChargerWatts} min={5} max={240} unit="W" onChange={(v) => alerts({ weakChargerWatts: v })} />
        )}
        <Toggle label="CPU-Drosselung" checked={a.throttling} onChange={(v) => alerts({ throttling: v })} />
        <Toggle label="Hohe Temperatur" checked={a.highTemp} onChange={(v) => alerts({ highTemp: v })} />
        {a.highTemp && (
          <NumberField label="ab" value={a.highTempCelsius} min={50} max={110} unit="°C" onChange={(v) => alerts({ highTempCelsius: v })} />
        )}
        <Toggle label="Expansion Card gesteckt/entfernt" checked={a.cardChanges} onChange={(v) => alerts({ cardChanges: v })} />
      </section>

      <div className="flex items-center gap-4">
        <button onClick={save} className="rounded-lg bg-accent px-5 py-2 font-semibold text-white shadow hover:brightness-110">
          Speichern
        </button>
        <span className="text-sm text-slate-500">{status}</span>
      </div>
    </div>
  );
}
