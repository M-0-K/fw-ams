import { setPowerMode, tunerReset, tunerSetConfig } from "../../api";
import type { ParamId } from "../../bindings/ParamId";
import type { PowerMode } from "../../bindings/PowerMode";
import type { ProfileSlot } from "../../bindings/ProfileSlot";
import type { TuneProfile } from "../../bindings/TuneProfile";
import type { TunerConfig } from "../../bindings/TunerConfig";
import type { TunerView } from "../../bindings/TunerView";
import { Row, Switch } from "../../components/controls";
import TuneTable from "../../components/TuneTable";
import { Button, Card, ConfirmButton, Notice } from "../../components/ui";
import { powerModes, slotLabel, type Run } from "./shared";

const POWER_IDS: ParamId[] = ["stapmW", "fastW", "slowW", "tctlC", "maxFreqMhz", "boost", "epp"];

/** Presets for the SMU power limits (Framework 13 AI 300 factory modes: 15/25, 25/33, 30/35 W) */
const presets: { label: string; desc: string; values: Partial<TuneProfile> }[] = [
  { label: "Standard", desc: "Firmware entscheidet", values: { stapmW: null, fastW: null, slowW: null, tctlC: null } },
  { label: "Leise", desc: "12 W / 20 W, 85 °C", values: { stapmW: 12, fastW: 20, slowW: 15, tctlC: 85 } },
  { label: "Effizienz", desc: "15 W / 25 W", values: { stapmW: 15, fastW: 25, slowW: 20, tctlC: null } },
  { label: "Ausgeglichen", desc: "25 W / 33 W", values: { stapmW: 25, fastW: 33, slowW: 30, tctlC: null } },
  { label: "Leistung", desc: "30 W / 35 W", values: { stapmW: 30, fastW: 35, slowW: 35, tctlC: null } },
];

/** Unsaved edits of the tuner config, kept by the page so tab switches do not lose them */
export type Draft = {
  value: TunerConfig | null;
  set: (c: TunerConfig | null) => void;
  slot: ProfileSlot | null;
  setSlot: (s: ProfileSlot) => void;
};

export default function Tuning({
  tab,
  view,
  draft,
  busy,
  run,
}: {
  tab: "power" | "voltage";
  view: TunerView;
  draft: Draft;
  busy: boolean;
  run: Run;
}) {
  const st = view.state;
  const config = draft.value ?? st.config;
  const editSlot = draft.slot ?? st.activeSlot;
  const dirty = draft.value !== null && JSON.stringify(draft.value) !== JSON.stringify(st.config);
  const smuReady = st.availability === "ready" || st.availability === "mock";
  const windowsReady = smuReady || st.specs.length > 0;

  const editValue = (id: ParamId, v: number | null) =>
    draft.set({ ...config, [editSlot]: { ...config[editSlot], [id]: v } });
  const apply = () =>
    run(() => tunerSetConfig(config), "Übernommen – Status siehe Tabelle").then((ok) => ok && draft.set(null));

  return (
    <>
      {tab === "power" && <PowerModeCard current={view.powerMode} run={run} />}
      <Card
        title={tab === "power" ? "Powerlimits, Temperatur und Takt" : "Spannung (Curve Optimizer)"}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Button onClick={() => draft.set(null)} disabled={!dirty || busy}>
              Verwerfen
            </Button>
            <ConfirmButton
              question="Alle Profile leeren und die Standardwerte wiederherstellen?"
              disabled={busy || !windowsReady}
              onConfirm={() => run(tunerReset, "Alle Werte auf Standard zurückgesetzt").then(() => draft.set(null))}
            >
              Alles auf Standard
            </ConfirmButton>
            <Button variant="primary" onClick={apply} disabled={!dirty || busy || !windowsReady}>
              Übernehmen
            </Button>
          </div>
        }
      >
        <SlotPicker active={st.activeSlot} value={editSlot} onChange={draft.setSlot} />
        {tab === "power" && (
          <div className="mb-4 flex flex-wrap gap-2">
            {presets.map((p) => (
              <button
                key={p.label}
                disabled={!smuReady}
                onClick={() => draft.set({ ...config, [editSlot]: { ...config[editSlot], ...p.values } })}
                className="rounded-lg border border-slate-300 px-3 py-1.5 text-left text-sm hover:border-accent disabled:opacity-40 dark:border-slate-600"
              >
                <div className="font-medium">{p.label}</div>
                <div className="text-xs text-slate-500">{p.desc}</div>
              </button>
            ))}
          </div>
        )}
        {tab === "voltage" && <VoltageNotice view={view} />}
        <TuneTable
          ids={tab === "power" ? POWER_IDS : ["coAll"]}
          specs={st.specs}
          params={st.params}
          draft={(id) => config[editSlot][id]}
          saved={(id) => st.config[editSlot][id]}
          editingActive={editSlot === st.activeSlot}
          onChange={editValue}
        />
        <OrderHint profile={config[editSlot]} />
        <div className="mt-4 divide-y divide-slate-100 border-t border-slate-100 dark:divide-slate-800 dark:border-slate-800">
          {tab === "power" ? (
            <>
              <Row
                title="Von Firmware überschriebene Werte wiederherstellen"
                desc="Die Framework-EC setzt Limits z. B. beim Ein-/Ausstecken des Netzteils zurück."
              >
                <Switch
                  label="Automatisch wiederherstellen"
                  checked={config.autoReapply}
                  onChange={(v) => draft.set({ ...config, autoReapply: v })}
                />
              </Row>
              <Row
                title="Nach dem Systemstart anwenden"
                desc="Sonst gelten nach einem Neustart die Firmware-Werte; Windows-Energieplanwerte werden dann zurückgesetzt."
              >
                <Switch
                  label="Nach dem Systemstart anwenden"
                  checked={config.applyOnBoot}
                  onChange={(v) => draft.set({ ...config, applyOnBoot: v })}
                />
              </Row>
            </>
          ) : (
            <Row
              title="Curve Optimizer nach dem Systemstart anwenden"
              desc="Erst einschalten, wenn der Wert über Tage stabil lief. Nach einem Absturz im Test wird er nie automatisch gesetzt."
            >
              <Switch
                label="Curve Optimizer nach dem Systemstart anwenden"
                checked={config.coOnBoot}
                onChange={(v) => draft.set({ ...config, coOnBoot: v })}
              />
            </Row>
          )}
        </div>
      </Card>
    </>
  );
}

function PowerModeCard({ current, run }: { current: PowerMode | null; run: Run }) {
  return (
    <Card title="Windows-Energiemodus">
      <p className="mb-3 text-sm text-slate-500">
        Wirkt ohne Hilfsdienst. Framework ordnet jedem Modus eigene Firmware-Powerlimits zu; eigene Limits unten
        überschreiben sie.
      </p>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3" role="radiogroup" aria-label="Windows-Energiemodus">
        {powerModes.map((m) => (
          <button
            key={m.id}
            role="radio"
            aria-checked={current === m.id}
            onClick={() => run(() => setPowerMode(m.id), `Energiemodus: ${m.label}`)}
            className={`rounded-lg border px-3 py-2 text-left text-sm transition ${
              current === m.id ? "border-accent bg-accent/10" : "border-slate-300 hover:border-accent dark:border-slate-600"
            }`}
          >
            <div className="font-medium">
              {current === m.id && "✓ "}
              {m.label}
            </div>
            <div className="text-xs text-slate-500">{m.desc}</div>
          </button>
        ))}
      </div>
    </Card>
  );
}

function SlotPicker({
  active,
  value,
  onChange,
}: {
  active: ProfileSlot;
  value: ProfileSlot;
  onChange: (s: ProfileSlot) => void;
}) {
  return (
    <div className="mb-4 flex flex-wrap items-center gap-3">
      <span className="text-sm text-slate-500">Profil bearbeiten:</span>
      <div className="flex rounded-lg bg-slate-100 p-0.5 dark:bg-slate-800" role="radiogroup" aria-label="Profil">
        {(["ac", "dc", "saver"] as ProfileSlot[]).map((s) => (
          <button
            key={s}
            role="radio"
            aria-checked={value === s}
            onClick={() => onChange(s)}
            className={`rounded-md px-3 py-1 text-sm transition ${
              value === s ? "bg-white font-medium shadow-sm dark:bg-slate-600" : "text-slate-500 hover:text-slate-900 dark:hover:text-slate-100"
            }`}
          >
            {slotLabel[s]}
            {active === s && <span className="ml-1.5 text-xs text-emerald-600 dark:text-emerald-400">● aktiv</span>}
          </button>
        ))}
      </div>
    </div>
  );
}

function OrderHint({ profile }: { profile: TuneProfile }) {
  const { stapmW, slowW, fastW } = profile;
  const bad =
    (stapmW !== null && slowW !== null && slowW < stapmW) ||
    (slowW !== null && fastW !== null && fastW < slowW) ||
    (stapmW !== null && fastW !== null && fastW < stapmW);
  if (!bad) return null;
  return (
    <p className="mt-2 text-sm text-orange-700 dark:text-orange-400">
      ⚠ Üblich ist Dauerleistung ≤ Boost (Slow) ≤ Kurzboost (Fast). Sonst greift das kleinere Limit.
    </p>
  );
}

function VoltageNotice({ view }: { view: TunerView }) {
  const trial = view.state.coTrial;
  return (
    <div className="mb-4 space-y-2">
      <Notice tone="warn">
        Ryzen-CPUs haben keinen festen Spannungsoffset; der Curve Optimizer verschiebt die Spannungs-Frequenz-Kurve. In
        kleinen Schritten (−5) testen. Zu viel führt zu Abstürzen – ein Neustart setzt den Wert immer zurück. Die
        Hardware meldet den Wert nicht zurück; die Wirkung siehst du an Spannung/Temperatur bei gleichem Takt.
      </Notice>
      {trial.kind === "running" && (
        <Notice tone="info">Testlauf: Der neue Wert gilt nach {trial.seconds} s ohne Absturz als stabil.</Notice>
      )}
      {trial.kind === "crashedLastTime" && (
        <Notice tone="error">
          ✕ Das System ist während des letzten Tests neu gestartet. Der Wert wurde nicht automatisch angewendet – bitte
          einen schwächeren Wert wählen.
        </Notice>
      )}
    </div>
  );
}
