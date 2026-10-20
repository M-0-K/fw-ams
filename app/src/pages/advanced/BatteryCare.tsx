import { chargeFullOnce, setSettings } from "../../api";
import type { ChargeControl } from "../../bindings/ChargeControl";
import type { ChargeParam } from "../../bindings/ChargeParam";
import type { Settings } from "../../bindings/Settings";
import type { Snapshot } from "../../bindings/Snapshot";
import { NumberInput, Row, Switch } from "../../components/controls";
import { StatusBadge } from "../../components/TuneTable";
import { Button, Card, Notice } from "../../components/ui";
import { num } from "../../format";
import type { Run } from "./shared";

const rates = [
  { value: 0, label: "Kein Limit" },
  { value: 0.7, label: "0,7 C" },
  { value: 0.5, label: "0,5 C (schonend)" },
  { value: 0.3, label: "0,3 C (sehr schonend)" },
];

function ChargeRow({ title, p, unit }: { title: string; p: ChargeParam | undefined; unit: string }) {
  if (!p) return null;
  return (
    <tr>
      <td className="py-2 pr-3 font-medium">{title}</td>
      <td className="px-3 py-2 text-right tabular-nums">{p.desired === null ? "Standard" : num(p.desired, 0, unit)}</td>
      <td className="px-3 py-2 text-right tabular-nums">{num(p.sent, 0, unit)}</td>
      <td className="px-3 py-2 text-right font-semibold tabular-nums">{num(p.actual, 0, unit)}</td>
      <td className="py-2 pl-3">
        <StatusBadge status={p.status} />
        {p.error && <div className="mt-1 text-xs text-slate-500">{p.error}</div>}
      </td>
    </tr>
  );
}

export default function BatteryCare({
  charge,
  settings,
  s,
  run,
}: {
  charge: ChargeControl | null;
  settings: Settings;
  s: Snapshot;
  run: Run;
}) {
  const b = settings.battery;
  const save = (patch: Partial<Settings["battery"]>) =>
    run(() => setSettings({ ...settings, battery: { ...b, ...patch } }), "Gespeichert ✓");
  const design = charge?.designMah ?? s.battery?.designMah ?? null;
  const voltage = s.battery?.voltageV ?? 15.5;

  return (
    <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
      <Card title="Ladelimit">
        <div className="divide-y divide-slate-100 dark:divide-slate-800">
          <Row
            title="Ladelimit von fw-ams verwalten"
            desc="Schreibt das Limit in den Embedded Controller (wie die BIOS-Einstellung) und setzt es nach Neustart/Netzteilwechsel erneut. Ausschalten stellt den vorherigen Wert wieder her."
          >
            <Switch label="Ladelimit verwalten" checked={b.manageChargeLimit} onChange={(v) => save({ manageChargeLimit: v })} />
          </Row>
          <Row title="Laden bis" desc="Für Dauerbetrieb am Netzteil sind 60–80 % akkuschonend.">
            <NumberInput
              value={b.chargeMax}
              min={25}
              max={100}
              unit="%"
              disabled={!b.manageChargeLimit}
              onCommit={(v) => save({ chargeMax: v })}
            />
          </Row>
          <Row
            title="Wieder laden unter"
            desc="Untere Grenze des Haltebereichs. Die aktuelle Framework-Firmware ignoriert diesen Wert laut Community noch."
          >
            <NumberInput
              value={b.chargeMin}
              min={0}
              max={Math.max(0, b.chargeMax - 5)}
              unit="%"
              disabled={!b.manageChargeLimit}
              onCommit={(v) => save({ chargeMin: v })}
            />
          </Row>
          <Row title="Einmalig auf 100 % laden" desc="Z. B. vor einer Reise. Beim nächsten Ladezyklus gilt das Limit wieder.">
            <Button disabled={!charge?.available} onClick={() => run(chargeFullOnce, "Lädt diesmal bis 100 %")}>
              Voll laden
            </Button>
          </Row>
        </div>
      </Card>

      <Card title="Ladestrom">
        <p className="mb-3 text-sm text-slate-500">
          Ein niedriger Ladestrom schont den Akku und hält ihn kühler, das Laden dauert dafür länger. Die EC vergisst den
          Wert beim Herunterfahren; fw-ams setzt ihn beim Start und beim Einstecken erneut.
        </p>
        <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Ladestrom">
          {rates.map((r) => {
            const ma = design !== null && r.value > 0 ? r.value * design : null;
            return (
              <button
                key={r.value}
                role="radio"
                aria-checked={b.chargeRate === r.value}
                onClick={() => save({ chargeRate: r.value })}
                className={`rounded-lg border px-3 py-2 text-left text-sm transition ${
                  b.chargeRate === r.value ? "border-accent bg-accent/10" : "border-slate-300 hover:border-accent dark:border-slate-600"
                }`}
              >
                <div className="font-medium">{r.label}</div>
                <div className="text-xs text-slate-500">
                  {ma !== null ? `${num(ma, 0, "mA")} ≈ ${num((ma / 1000) * voltage, 0, "W")}` : "EC-Standard"}
                </div>
              </button>
            );
          })}
        </div>
      </Card>

      <Card title="Status (Embedded Controller)">
        {!charge?.available && (
          <Notice tone="warn" className="mb-2">
            Kein EC-Zugriff – Werte werden gesendet, sobald er da ist.
          </Notice>
        )}
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-200 text-left text-xs uppercase tracking-wide text-slate-500 dark:border-slate-800">
              <th className="py-2 pr-3 font-medium">Wert</th>
              <th className="px-3 py-2 text-right font-medium">Eingestellt</th>
              <th className="px-3 py-2 text-right font-medium">Gesendet</th>
              <th className="px-3 py-2 text-right font-medium">Ist (EC)</th>
              <th className="py-2 pl-3 font-medium">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
            <ChargeRow title="Laden bis" p={charge?.max} unit="%" />
            <ChargeRow title="Wieder laden unter" p={charge?.min} unit="%" />
            <ChargeRow title="Ladestrom" p={charge?.current} unit="mA" />
          </tbody>
        </table>
        <div className="mt-2 space-y-0.5 text-xs text-slate-500">
          {charge?.originalMax != null && (
            <p>Vor fw-ams: Laden bis {charge.originalMax} % – wird beim Ausschalten der Verwaltung wiederhergestellt.</p>
          )}
          {charge?.fullOnceMs != null && (
            <p>„Voll laden“ ausgelöst um {new Date(charge.fullOnceMs).toLocaleTimeString("de-DE")}.</p>
          )}
        </div>
      </Card>

      <Card title="Niedriger Akkustand">
        <p className="text-sm text-slate-500">
          Warnung bei {settings.alerts.batteryLowPercent} %, kritisch bei {settings.alerts.batteryCriticalPercent} %.
          Automatischer Sparmodus: {b.saver ? `ab ${b.saverPercent} %` : "aus"}. Einstellbar unter Einstellungen.
        </p>
      </Card>
    </div>
  );
}
