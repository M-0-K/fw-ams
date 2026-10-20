import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";
import { NumberInput, Row, Switch } from "../components/controls";
import ResetPanel from "../components/ResetPanel";
import { Button, ConfirmButton, Toast, useFlash } from "../components/ui";
import type { AlertSettings } from "../bindings/AlertSettings";
import type { BatterySettings } from "../bindings/BatterySettings";
import type { Settings } from "../bindings/Settings";
import type { Theme } from "../bindings/Theme";
import { forgetCards, setSettings, useSettings } from "../api";

const themes: { id: Theme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "light", label: "Hell" },
  { id: "dark", label: "Dunkel" },
];

function Section({ title, actions, children }: { title: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="card divide-y divide-slate-100 dark:divide-slate-800">
      <div className="flex flex-wrap items-center justify-between gap-2 pb-2">
        <h2 className="font-semibold">{title}</h2>
        {actions}
      </div>
      {children}
    </section>
  );
}

export default function SettingsPage() {
  const live = useSettings();
  const [settings, setLocal] = useState<Settings | null>(null);
  const [msg, flash] = useFlash();
  const [panel, setPanel] = useState<"disable" | "factory" | null>(null);

  // Follow the backend, e.g. after a reset or a change on the Advanced page
  useEffect(() => {
    if (live) setLocal(live);
  }, [live]);

  if (!settings) return <div className="card text-slate-400">Lade Einstellungen…</div>;

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
  const b = settings.battery;
  const battery = (patch: Partial<BatterySettings>) => update({ battery: { ...b, ...patch } });

  const testNotification = () =>
    invoke("test_notification")
      .then(() => flash("Test-Benachrichtigung gesendet"))
      .catch((e) => flash(`Benachrichtigung fehlgeschlagen: ${e}`, true));

  return (
    <div className="max-w-2xl space-y-4">
      <Section title="Darstellung">
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
      </Section>

      <Section title="Allgemein">
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
      </Section>

      <Section title="Erweiterte Funktionen">
        <Row
          title="CPU-Tuning, Akkupflege und Sparmodus"
          desc={
            settings.advancedFeatures
              ? "Funktionen, die das System verändern: Powerlimits, Ladelimit, Ladestrom, Energiemodus. Ausschalten stellt alles auf den Stand vor fw-ams zurück."
              : "Aus: fw-ams überwacht nur und verändert nichts am System."
          }
        >
          <Switch
            label="Erweiterte Funktionen"
            checked={settings.advancedFeatures}
            disabled={panel !== null}
            onChange={(v) => (v ? update({ advancedFeatures: true }) : setPanel("disable"))}
          />
        </Row>
        {panel === "disable" && <ResetPanel factory={false} onClose={() => setPanel(null)} />}
      </Section>

      {settings.advancedFeatures && (
        <Section title="Energiesparen bei niedrigem Akku">
          <Row
            title="Automatischer Sparmodus"
            desc="Im Akkubetrieb unter dem Schwellwert sparsamer arbeiten; ab 5 % darüber oder am Netzteil wieder normal."
          >
            <NumberInput
              value={b.saverPercent}
              min={5}
              max={50}
              unit="%"
              disabled={!b.saver}
              onCommit={(v) => battery({ saverPercent: v })}
            />
            <Switch label="Automatischer Sparmodus" checked={b.saver} onChange={(v) => battery({ saver: v })} />
          </Row>
          <Row
            title="Windows-Energiemodus umschalten"
            desc="„Beste Energieeffizienz“ (Framework: 15 W dauerhaft), danach den vorherigen Modus wiederherstellen."
          >
            <Switch
              label="Windows-Energiemodus umschalten"
              checked={b.saverPowerMode}
              disabled={!b.saver}
              onChange={(v) => battery({ saverPowerMode: v })}
            />
          </Row>
          <div className="py-3 text-xs text-slate-500">
            Zusätzlich gilt das Sparprofil unter Erweitert → Leistung &amp; Takt, sofern der Hilfsdienst installiert ist.
            Ladelimit und Ladestrom stellst du unter Erweitert → Akkupflege ein.
          </div>
        </Section>
      )}

      <Section title="Benachrichtigungen" actions={<Button onClick={testNotification}>Test-Benachrichtigung senden</Button>}>
        <Row title="Akku fast leer" desc="Warnung beim Entladen unter dem Schwellwert.">
          <NumberInput
            value={a.batteryLowPercent}
            min={2}
            max={99}
            unit="%"
            disabled={!a.batteryLow}
            onCommit={(v) => alerts({ batteryLowPercent: v })}
          />
          <Switch label="Akku fast leer" checked={a.batteryLow} onChange={(v) => alerts({ batteryLow: v })} />
        </Row>
        <Row title="Akku kritisch" desc="Zweite, dringende Warnung. Muss unter der ersten Stufe liegen.">
          <NumberInput
            value={a.batteryCriticalPercent}
            min={1}
            max={a.batteryLowPercent - 1}
            unit="%"
            disabled={!a.batteryCritical}
            onCommit={(v) => alerts({ batteryCriticalPercent: v })}
          />
          <Switch label="Akku kritisch" checked={a.batteryCritical} onChange={(v) => alerts({ batteryCritical: v })} />
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
      </Section>

      <Section title="Expansion Cards">
        <Row
          title="Gemerkte Karten"
          desc="fw-ams merkt sich, welche Karte in welchem Slot steckt. Passive USB-A- und USB-C-Karten werden so auch angezeigt, wenn sie sich gerade nicht melden. Nach dem Umstecken hier vergessen."
        >
          <ConfirmButton
            question="Alle vergessen?"
            onConfirm={() =>
              forgetCards()
                .then(() => flash("Gemerkte Karten gelöscht"))
                .catch((e) => flash(String(e), true))
            }
          >
            Alle vergessen
          </ConfirmButton>
        </Row>
      </Section>

      <Section title="Zurücksetzen">
        <Row
          title="Werkseinstellungen"
          desc="Macht alle Änderungen von fw-ams am System rückgängig und setzt die App auf den Auslieferungszustand."
        >
          <Button variant="danger" disabled={panel !== null} onClick={() => setPanel("factory")}>
            Zurücksetzen…
          </Button>
        </Row>
        {panel === "factory" && <ResetPanel factory onClose={() => setPanel(null)} />}
      </Section>

      <Toast msg={msg} />
    </div>
  );
}
