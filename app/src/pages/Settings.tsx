import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { AlertSettings } from "../bindings/AlertSettings";
import type { Settings } from "../bindings/Settings";
import type { Theme } from "../bindings/Theme";
import { getSettings, setSettings } from "../api";

function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full transition disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-accent ${
        checked ? "bg-accent" : "bg-slate-300 dark:bg-slate-600"
      }`}
    >
      <span
        className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-5" : ""
        }`}
      />
    </button>
  );
}

const show = (v: number) => String(v).replace(".", ",");

/** Number field that only commits valid values: empty or invalid input falls back to the old value */
function NumberInput({
  value,
  min,
  max,
  unit,
  disabled,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  disabled?: boolean;
  onCommit: (v: number) => void;
}) {
  const [text, setText] = useState(show(value));
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    setText(show(value));
  }, [value]);

  const commit = (raw: string) => {
    window.clearTimeout(timer.current);
    const parsed = Number(raw.replace(",", "."));
    if (raw.trim() === "" || !Number.isFinite(parsed)) {
      setText(show(value));
      return;
    }
    const clamped = Math.min(max, Math.max(min, parsed));
    setText(show(clamped));
    if (clamped !== value) onCommit(clamped);
  };

  return (
    <span className={`flex items-center gap-1.5 text-sm ${disabled ? "opacity-40" : ""}`}>
      <input
        inputMode="decimal"
        value={text}
        disabled={disabled}
        aria-label={`Wert in ${unit}`}
        onChange={(e) => {
          setText(e.target.value);
          window.clearTimeout(timer.current);
          const next = e.target.value;
          timer.current = window.setTimeout(() => commit(next), 800);
        }}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        className="w-16 rounded-md border border-slate-300 bg-transparent px-2 py-1 text-right tabular-nums focus:border-accent focus:outline-none dark:border-slate-600"
      />
      <span className="w-6 text-slate-500">{unit}</span>
    </span>
  );
}

function Row({ title, desc, children }: { title: string; desc?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {desc && <div className="text-xs text-slate-500">{desc}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-4">{children}</div>
    </div>
  );
}

const themes: { id: Theme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "light", label: "Hell" },
  { id: "dark", label: "Dunkel" },
];

export default function SettingsPage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [status, setStatus] = useState<{ text: string; error?: boolean }>({ text: "" });
  const statusTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    getSettings()
      .then(setLocal)
      .catch((e) => setStatus({ text: `Fehler: ${e}`, error: true }));
  }, []);

  if (!settings) return <div className="card text-slate-400">{status.text || "Lade Einstellungen…"}</div>;

  const flash = (text: string, error = false) => {
    setStatus({ text, error });
    window.clearTimeout(statusTimer.current);
    statusTimer.current = window.setTimeout(() => setStatus({ text: "" }), error ? 6000 : 2000);
  };

  // Every change is saved right away
  const update = (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setLocal(next);
    setSettings(next)
      .then(() => flash("Gespeichert ✓"))
      .catch((e) => flash(`Fehler beim Speichern: ${e}`, true));
  };
  const a = settings.alerts;
  const alerts = (patch: Partial<AlertSettings>) => update({ alerts: { ...a, ...patch } });

  const testNotification = () =>
    invoke("test_notification")
      .then(() => flash("Test-Benachrichtigung gesendet"))
      .catch((e) => flash(`Benachrichtigung fehlgeschlagen: ${e}`, true));

  return (
    <div className="max-w-2xl space-y-4">
      <div className="flex h-6 items-center justify-end text-sm">
        <span className={status.error ? "text-red-500" : "text-slate-500"} role="status">
          {status.text}
        </span>
      </div>

      <section className="card divide-y divide-slate-100 dark:divide-slate-800">
        <h2 className="pb-2 font-semibold">Darstellung</h2>
        <Row title="Farbschema" desc="System folgt der Windows- bzw. Linux-Einstellung.">
          <div className="flex rounded-lg bg-slate-100 p-0.5 dark:bg-slate-800" role="radiogroup" aria-label="Farbschema">
            {themes.map((t) => (
              <button
                key={t.id}
                role="radio"
                aria-checked={settings.theme === t.id}
                onClick={() => update({ theme: t.id })}
                className={`rounded-md px-3 py-1 text-sm transition ${
                  settings.theme === t.id
                    ? "bg-white font-medium shadow-sm dark:bg-slate-600"
                    : "text-slate-500 hover:text-slate-900 dark:hover:text-slate-100"
                }`}
              >
                {t.label}
              </button>
            ))}
          </div>
        </Row>
      </section>

      <section className="card divide-y divide-slate-100 dark:divide-slate-800">
        <h2 className="pb-2 font-semibold">Allgemein</h2>
        <Row title="Mit dem System starten" desc="fw-ams startet bei der Anmeldung.">
          <Switch label="Mit dem System starten" checked={settings.autostart} onChange={(v) => update({ autostart: v })} />
        </Row>
        <Row title="Beim Autostart nur im Tray starten" desc="Das Fenster bleibt zu, nur das Tray-Symbol erscheint.">
          <Switch
            label="Beim Autostart nur im Tray starten"
            checked={settings.startMinimized}
            disabled={!settings.autostart}
            onChange={(v) => update({ startMinimized: v })}
          />
        </Row>
        <Row title="Aktualisierung" desc="Wie oft die Hardware abgefragt wird (0,5 bis 60 s).">
          <NumberInput
            value={settings.pollIntervalMs / 1000}
            min={0.5}
            max={60}
            unit="s"
            onCommit={(v) => update({ pollIntervalMs: Math.round(v * 1000) })}
          />
        </Row>
      </section>

      <section className="card divide-y divide-slate-100 dark:divide-slate-800">
        <div className="flex items-center justify-between pb-2">
          <h2 className="font-semibold">Benachrichtigungen</h2>
          <button
            onClick={testNotification}
            className="rounded-lg border border-slate-300 px-3 py-1 text-sm hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
          >
            Test-Benachrichtigung senden
          </button>
        </div>
        <Row title="Akku fast leer" desc="Warnung beim Entladen unter dem Schwellwert.">
          <NumberInput
            value={a.batteryLowPercent}
            min={1}
            max={99}
            unit="%"
            disabled={!a.batteryLow}
            onCommit={(v) => alerts({ batteryLowPercent: v })}
          />
          <Switch label="Akku fast leer" checked={a.batteryLow} onChange={(v) => alerts({ batteryLow: v })} />
        </Row>
        <Row title="Ladelimit erreicht" desc="Die Ladung pausiert am eingestellten Limit.">
          <Switch label="Ladelimit erreicht" checked={a.chargeLimitReached} onChange={(v) => alerts({ chargeLimitReached: v })} />
        </Row>
        <Row title="Schwaches Ladegerät" desc="Das Netzteil liefert weniger als den Schwellwert.">
          <NumberInput
            value={a.weakChargerWatts}
            min={5}
            max={240}
            unit="W"
            disabled={!a.weakCharger}
            onCommit={(v) => alerts({ weakChargerWatts: v })}
          />
          <Switch label="Schwaches Ladegerät" checked={a.weakCharger} onChange={(v) => alerts({ weakCharger: v })} />
        </Row>
        <Row title="CPU-Drosselung" desc="Der Embedded Controller bremst die CPU aus.">
          <Switch label="CPU-Drosselung" checked={a.throttling} onChange={(v) => alerts({ throttling: v })} />
        </Row>
        <Row title="Hohe Temperatur" desc="Ein Sensor erreicht den Schwellwert.">
          <NumberInput
            value={a.highTempCelsius}
            min={50}
            max={110}
            unit="°C"
            disabled={!a.highTemp}
            onCommit={(v) => alerts({ highTempCelsius: v })}
          />
          <Switch label="Hohe Temperatur" checked={a.highTemp} onChange={(v) => alerts({ highTemp: v })} />
        </Row>
        <Row title="Expansion Card gesteckt/entfernt" desc="Meldung, sobald sich ein Slot ändert.">
          <Switch label="Expansion Card gesteckt/entfernt" checked={a.cardChanges} onChange={(v) => alerts({ cardChanges: v })} />
        </Row>
      </section>
    </div>
  );
}
