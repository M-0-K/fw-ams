import { useState, type ReactNode } from "react";
import type { ChargeControl } from "../bindings/ChargeControl";
import type { ChargeParam } from "../bindings/ChargeParam";
import type { HistorySample } from "../bindings/HistorySample";
import type { ParamId } from "../bindings/ParamId";
import type { PowerMode } from "../bindings/PowerMode";
import type { ProfileSlot } from "../bindings/ProfileSlot";
import type { Settings } from "../bindings/Settings";
import type { SmuTelemetry } from "../bindings/SmuTelemetry";
import type { Snapshot } from "../bindings/Snapshot";
import type { TuneProfile } from "../bindings/TuneProfile";
import type { TunerConfig } from "../bindings/TunerConfig";
import type { TunerView } from "../bindings/TunerView";
import {
  chargeFullOnce,
  openPawnioSite,
  setPowerMode,
  setSettings,
  tunerInstall,
  tunerReset,
  tunerSetConfig,
  tunerUninstall,
  useChargeControl,
  useSettings,
  useTuner,
} from "../api";
import { NumberInput, Row, Switch } from "../components/controls";
import LineChart from "../components/LineChart";
import PowerBreakdown from "../components/PowerBreakdown";
import Stat from "../components/Stat";
import TuneTable, { StatusBadge } from "../components/TuneTable";
import { num } from "../format";

const tabs = [
  { id: "live", label: "Live-Analyse" },
  { id: "power", label: "Leistung & Takt" },
  { id: "voltage", label: "Spannung" },
  { id: "battery", label: "Akkupflege" },
  { id: "events", label: "Ereignisse" },
] as const;
type Tab = (typeof tabs)[number]["id"];

const slotLabel: Record<ProfileSlot, string> = { ac: "Netzteil", dc: "Akku", saver: "Energiesparen" };

const POWER_IDS: ParamId[] = ["stapmW", "fastW", "slowW", "tctlC", "maxFreqMhz", "boost", "epp"];

/** Presets for the SMU power limits (Framework 13 AI 300 factory modes: 15/25, 25/33, 30/35 W) */
const presets: { label: string; desc: string; values: Partial<TuneProfile> }[] = [
  { label: "Standard", desc: "Firmware entscheidet", values: { stapmW: null, fastW: null, slowW: null, tctlC: null } },
  { label: "Leise", desc: "12 W / 20 W, 85 °C", values: { stapmW: 12, fastW: 20, slowW: 15, tctlC: 85 } },
  { label: "Effizienz", desc: "15 W / 25 W", values: { stapmW: 15, fastW: 25, slowW: 20, tctlC: null } },
  { label: "Ausgeglichen", desc: "25 W / 33 W", values: { stapmW: 25, fastW: 33, slowW: 30, tctlC: null } },
  { label: "Leistung", desc: "30 W / 35 W", values: { stapmW: 30, fastW: 35, slowW: 35, tctlC: null } },
];

const powerModes: { id: PowerMode; label: string; desc: string }[] = [
  { id: "efficiency", label: "Beste Energieeffizienz", desc: "Framework: 15 W / 25 W" },
  { id: "balanced", label: "Ausbalanciert", desc: "Framework: 25 W / 33 W" },
  { id: "performance", label: "Beste Leistung", desc: "Framework: 30 W / 35 W" },
];

function Card({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <section className="card">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-semibold">{title}</h2>
        {actions}
      </div>
      {children}
    </section>
  );
}

function Button({
  children,
  onClick,
  primary,
  disabled,
}: {
  children: ReactNode;
  onClick: () => void;
  primary?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`rounded-lg px-3 py-1.5 text-sm font-medium transition disabled:opacity-40 ${
        primary
          ? "bg-accent text-white hover:brightness-110"
          : "border border-slate-300 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800"
      }`}
    >
      {children}
    </button>
  );
}

/** Which limit the APU is running into right now */
function limiters(t: SmuTelemetry | null | undefined): string[] {
  if (!t) return [];
  const hit = (v: number | null, l: number | null) => v !== null && l !== null && l > 0 && v >= l * 0.97;
  const out: string[] = [];
  if (hit(t.stapm.value, t.stapm.limit)) out.push("Dauerleistung (STAPM)");
  if (hit(t.slow.value, t.slow.limit)) out.push("Boost (PPT Slow)");
  if (hit(t.fast.value, t.fast.limit)) out.push("Kurzboost (PPT Fast)");
  if (hit(t.tctl.value, t.tctl.limit)) out.push("Temperaturgrenze (Tctl)");
  return out;
}

export default function AdvancedPage({ s }: { s: Snapshot }) {
  const { view, history } = useTuner();
  const charge = useChargeControl();
  const settings = useSettings();
  const [tab, setTab] = useState<Tab>("live");
  const [draft, setDraft] = useState<TunerConfig | null>(null);
  const [slot, setSlot] = useState<ProfileSlot | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; error?: boolean } | null>(null);

  if (!view) return <div className="card text-slate-400">Verbinde mit dem Tuner…</div>;

  const st = view.state;
  const config = draft ?? st.config;
  const editSlot = slot ?? st.activeSlot;
  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(st.config);
  const smuReady = st.availability === "ready" || st.availability === "mock";
  const windowsReady = smuReady || st.specs.length > 0;

  const run = async (what: () => Promise<unknown>, ok: string) => {
    setBusy(true);
    setMsg(null);
    try {
      await what();
      setMsg({ text: ok });
      return true;
    } catch (e) {
      setMsg({ text: String(e), error: true });
      return false;
    } finally {
      setBusy(false);
    }
  };

  const editValue = (id: ParamId, v: number | null) =>
    setDraft({ ...config, [editSlot]: { ...config[editSlot], [id]: v } });
  const apply = () =>
    run(() => tunerSetConfig(config), "Übernommen – Status siehe Tabelle").then((ok) => ok && setDraft(null));

  const last = history.length ? history[history.length - 1] : null;

  return (
    <div className="space-y-4">
      <Header view={view} s={s} />
      <Setup view={view} busy={busy} run={run} />

      <div className="flex flex-wrap items-center gap-1 border-b border-slate-200 dark:border-slate-800" role="tablist">
        {tabs.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={`-mb-px border-b-2 px-3 py-2 text-sm font-medium transition ${
              tab === t.id
                ? "border-accent text-accent"
                : "border-transparent text-slate-500 hover:text-slate-900 dark:hover:text-slate-100"
            }`}
          >
            {t.label}
          </button>
        ))}
        <span className={`ml-auto text-sm ${msg?.error ? "text-red-500" : "text-slate-500"}`} role="status">
          {busy ? "Bitte warten…" : msg?.text}
        </span>
      </div>

      {tab === "live" && <Live view={view} history={history} last={last} />}

      {(tab === "power" || tab === "voltage") && (
        <>
          {tab === "power" && <PowerModeCard current={view.powerMode} run={run} />}
          <Card
            title={tab === "power" ? "Powerlimits, Temperatur und Takt" : "Spannung (Curve Optimizer)"}
            actions={
              <div className="flex flex-wrap items-center gap-2">
                <Button onClick={() => setDraft(null)} disabled={!dirty || busy}>
                  Verwerfen
                </Button>
                <Button
                  onClick={() =>
                    window.confirm("Alle Profile leeren und die Standardwerte der Firmware wiederherstellen?") &&
                    run(tunerReset, "Alle Werte auf Standard zurückgesetzt").then(() => setDraft(null))
                  }
                  disabled={busy || !windowsReady}
                >
                  Alles auf Standard
                </Button>
                <Button primary onClick={apply} disabled={!dirty || busy || !windowsReady}>
                  Übernehmen
                </Button>
              </div>
            }
          >
            <SlotPicker active={st.activeSlot} value={editSlot} onChange={setSlot} />
            {tab === "power" && (
              <div className="mb-4 flex flex-wrap gap-2">
                {presets.map((p) => (
                  <button
                    key={p.label}
                    disabled={!smuReady}
                    onClick={() => setDraft({ ...config, [editSlot]: { ...config[editSlot], ...p.values } })}
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
                      onChange={(v) => setDraft({ ...config, autoReapply: v })}
                    />
                  </Row>
                  <Row title="Nach dem Systemstart anwenden" desc="Sonst gelten nach einem Neustart die Firmware-Werte.">
                    <Switch
                      label="Nach dem Systemstart anwenden"
                      checked={config.applyOnBoot}
                      onChange={(v) => setDraft({ ...config, applyOnBoot: v })}
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
                    onChange={(v) => setDraft({ ...config, coOnBoot: v })}
                  />
                </Row>
              )}
            </div>
          </Card>
        </>
      )}

      {tab === "battery" && settings && <BatteryCare charge={charge} settings={settings} s={s} run={run} />}

      {tab === "events" && <Events view={view} />}
    </div>
  );
}

function Header({ view, s }: { view: TunerView; s: Snapshot }) {
  const st = view.state;
  const cpu = st.cpu;
  return (
    <div className="card grid grid-cols-2 gap-4 md:grid-cols-5">
      <Stat label="CPU" value={<span className="text-base">{cpu?.name ?? "n/v"}</span>} sub={cpu ? `${cpu.family} · ${cpu.cores} Kerne / ${cpu.threads} Threads` : undefined} />
      <Stat
        label="SMU"
        value={cpu?.smuVersion ?? "n/v"}
        sub={
          cpu?.pmTableVersion != null
            ? `PM-Tabelle 0x${cpu.pmTableVersion.toString(16).toUpperCase()}${cpu.pmTableFull ? "" : " (Kernwerte unbekannt)"}`
            : undefined
        }
      />
      <Stat label="Energiequelle" value={s.charger.acPresent ? "Netzteil" : "Akku"} />
      <Stat label="Aktives Profil" value={slotLabel[st.activeSlot]} />
      <Stat label="Windows-Energiemodus" value={view.powerMode ? modeLabel(view.powerMode) : "n/v"} />
    </div>
  );
}

const modeLabel = (m: PowerMode) => powerModes.find((p) => p.id === m)?.label ?? "Anderer";

type Run = (what: () => Promise<unknown>, ok: string) => Promise<boolean>;

function Setup({ view, busy, run }: { view: TunerView; busy: boolean; run: Run }) {
  const st = view.state;
  const setup = view.setup;
  if (st.availability === "ready" || st.availability === "mock") {
    return (
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-emerald-300 bg-emerald-50 px-4 py-2 text-sm text-emerald-900 dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-200">
        <span>
          ✓ {st.availability === "mock" ? "Simulierte Hardware (--mock)" : `Hilfsdienst ${st.serviceVersion ?? ""} aktiv, SMU-Zugriff über PawnIO`}
        </span>
        {st.availability === "ready" && (
          <button
            className="text-xs underline"
            disabled={busy}
            onClick={() =>
              window.confirm("Hilfsdienst entfernen? Getunte Werte bleiben bis zum Neustart aktiv.") &&
              run(tunerUninstall, "Hilfsdienst entfernt")
            }
          >
            Dienst entfernen
          </button>
        )}
      </div>
    );
  }
  const step = (done: boolean, text: ReactNode) => (
    <li className="flex gap-2">
      <span className={done ? "text-emerald-600 dark:text-emerald-400" : "text-slate-400"} aria-hidden>
        {done ? "✓" : "○"}
      </span>
      <span>{text}</span>
    </li>
  );
  return (
    <div className="rounded-xl border border-amber-300 bg-amber-50 p-4 text-sm text-amber-950 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-100">
      <div className="mb-2 font-semibold">Erweiterte CPU-Funktionen einrichten</div>
      <p className="mb-3">{st.message}</p>
      {setup.platformSupported && (
        <>
          <ol className="mb-3 space-y-1.5">
            {step(
              setup.pawnioInstalled,
              <>
                PawnIO-Treiber installieren (signierter Nachfolger von WinRing0).{" "}
                {!setup.pawnioInstalled && (
                  <button className="underline" onClick={() => run(openPawnioSite, "pawnio.eu geöffnet")}>
                    pawnio.eu öffnen
                  </button>
                )}
              </>,
            )}
            {step(
              setup.moduleFound,
              setup.moduleFound
                ? "RyzenSMU-Modul gefunden"
                : "RyzenSMU-Modul fehlt (Entwicklung: scripts/fetch-pawnio-module.ps1 ausführen)",
            )}
            {step(
              setup.serviceRunning,
              <>
                fw-ams-Hilfsdienst installieren (eine Admin-Abfrage). Er nimmt nur geprüfte, begrenzte Werte an.
                {!setup.serviceExeFound && " – fw-ams-service.exe nicht gefunden (cargo build -p fw-ams-service)."}
              </>,
            )}
          </ol>
          <div className="flex flex-wrap items-center gap-2">
            <Button primary disabled={busy || !setup.serviceExeFound} onClick={() => run(tunerInstall, "Hilfsdienst installiert")}>
              {setup.serviceInstalled ? "Hilfsdienst neu installieren" : "Hilfsdienst installieren"}
            </Button>
            <span className="text-xs">
              Hinweis: Manche Anti-Cheat-Systeme in Spielen bemängeln geladene Low-Level-Treiber wie PawnIO.
            </span>
          </div>
        </>
      )}
      <p className="mt-3 text-xs">
        Ohne Dienst funktionieren weiterhin: Windows-Energiemodus, Live-Takt und -Auslastung, Akkupflege.
      </p>
    </div>
  );
}

function Live({ view, history, last }: { view: TunerView; history: HistorySample[]; last: HistorySample | null }) {
  const t = view.state.telemetry;
  const os = view.os;
  const limits = limiters(t);
  const throttleKnown = !!t;

  return (
    <div className="space-y-4">
      <div className="card grid grid-cols-2 gap-4 md:grid-cols-6">
        <Stat label="APU jetzt" value={num(last?.apuW, 1, "W")} sub="PPT Fast, Ist-Wert" />
        <Stat label="APU gemittelt" value={num(t?.stapm.value, 1, "W")} sub={`Limit ${num(t?.stapm.limit, 1, "W")}`} />
        <Stat label="CPU-Temperatur" value={num(t?.tctl.value, 0, "°C")} sub={`Grenze ${num(t?.tctl.limit, 0, "°C")}`} />
        <Stat label="CPU-Last" value={num(os.usagePercent, 0, "%")} />
        <Stat label="Takt Ø / max" value={`${num(last?.avgMhz, 0)} / ${num(last?.maxMhz, 0, "MHz")}`} sub={`Basis ${num(os.baseMhz, 0, "MHz")}`} />
        <Stat
          label="Begrenzt durch"
          value={<span className="text-base">{!throttleKnown ? "n/v" : limits.length ? limits.join(", ") : "nichts"}</span>}
          sub={throttleKnown ? "Ist-Wert ≥ 97 % des Limits" : "braucht SMU-Zugriff"}
        />
      </div>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <section className="card space-y-6">
          <LineChart
            title="Leistungsaufnahme"
            unit="W"
            samples={history}
            series={[
              { label: "APU jetzt", color: "var(--series-1)", get: (x) => x.apuW },
              { label: "APU gemittelt", color: "var(--series-2)", get: (x) => x.stapmW },
              { label: "Gesamtsystem", color: "var(--series-3)", get: (x) => x.systemW },
            ]}
            refs={[
              { label: "STAPM-Limit", get: (x) => x.stapmLimit },
              { label: "Fast-Limit", get: (x) => x.fastLimit },
            ]}
          />
          <LineChart
            title="CPU-Temperatur (Tctl)"
            unit="°C"
            digits={0}
            samples={history}
            series={[{ label: "Tctl", color: "var(--series-1)", get: (x) => x.tctlC }]}
            refs={[{ label: "Grenze", get: (x) => x.tctlLimit }]}
          />
          <LineChart
            title="CPU-Takt"
            unit="MHz"
            digits={0}
            samples={history}
            series={[
              { label: "Höchster Kern", color: "var(--series-1)", get: (x) => x.maxMhz },
              { label: "Durchschnitt", color: "var(--series-2)", get: (x) => x.avgMhz },
            ]}
          />
        </section>
        <div className="space-y-4">
          <Card title="Wohin geht die Leistung?">
            <PowerBreakdown sample={last} />
          </Card>
          <Cores view={view} />
        </div>
      </div>
    </div>
  );
}

function Cores({ view }: { view: TunerView }) {
  const cores = view.state.telemetry?.cores ?? [];
  const os = view.os;
  if (cores.length > 0) {
    return (
      <Card title="Kerne (SMU)">
        <div className="grid grid-cols-2 gap-2">
          {cores.map((c, i) => (
            <div key={i} className="rounded-lg bg-slate-50 p-2 text-xs tabular-nums dark:bg-slate-800/60">
              <div className="mb-1 font-semibold">Kern {i}</div>
              <div>{num(c.mhz, 0, "MHz")}</div>
              <div className="text-slate-500">
                {num(c.volt, 3, "V")} · {num(c.powerW, 2, "W")} · {num(c.tempC, 0, "°C")}
              </div>
            </div>
          ))}
        </div>
      </Card>
    );
  }
  const threads = os.threadMhz.length;
  return (
    <Card title="Threads (Windows)">
      {threads === 0 ? (
        <div className="text-sm text-slate-400">Keine Leistungsindikatoren verfügbar.</div>
      ) : (
        <div className="space-y-1.5">
          {os.threadMhz.map((mhz, i) => (
            <div key={i} className="grid grid-cols-[3rem_1fr_4.5rem] items-center gap-2 text-xs tabular-nums">
              <span className="text-slate-500">#{i}</span>
              <div className="h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                <div
                  className="h-full rounded-full"
                  style={{ width: `${Math.min(100, os.threadUsage[i] ?? 0)}%`, background: "var(--series-1)", transition: "width 0.6s" }}
                />
              </div>
              <span className="text-right">{num(mhz, 0, "MHz")}</span>
            </div>
          ))}
          <p className="pt-2 text-xs text-slate-500">
            Balken = Auslastung, Zahl = effektiver Takt (Windows-Leistungsindikatoren). Spannung und Leistung pro Kern
            sind für diese PM-Tabelle noch nicht zugeordnet.
          </p>
        </div>
      )}
    </Card>
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
              current === m.id
                ? "border-accent bg-accent/10"
                : "border-slate-300 hover:border-accent dark:border-slate-600"
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
      <div className="rounded-lg border border-orange-300 bg-orange-50 p-3 text-sm text-orange-950 dark:border-orange-800 dark:bg-orange-950 dark:text-orange-100">
        Ryzen-CPUs haben keinen festen Spannungsoffset; der Curve Optimizer verschiebt die Spannungs-Frequenz-Kurve. In
        kleinen Schritten (−5) testen. Zu viel führt zu Abstürzen – ein Neustart setzt den Wert immer zurück. Die
        Hardware meldet den Wert nicht zurück; die Wirkung siehst du an Spannung/Temperatur bei gleichem Takt.
      </div>
      {trial.kind === "running" && (
        <div className="rounded-lg bg-sky-50 p-3 text-sm text-sky-900 dark:bg-sky-950 dark:text-sky-200">
          Testlauf: Der neue Wert gilt nach {trial.seconds} s ohne Absturz als stabil.
        </div>
      )}
      {trial.kind === "crashedLastTime" && (
        <div className="rounded-lg bg-red-50 p-3 text-sm text-red-900 dark:bg-red-950 dark:text-red-200">
          ✕ Das System ist während des letzten Tests neu gestartet. Der Wert wurde nicht automatisch angewendet – bitte
          einen schwächeren Wert wählen.
        </div>
      )}
    </div>
  );
}

const rates = [
  { value: 0, label: "Kein Limit" },
  { value: 0.7, label: "0,7 C" },
  { value: 0.5, label: "0,5 C (schonend)" },
  { value: 0.3, label: "0,3 C (sehr schonend)" },
];

function ChargeRow({ title, p, unit }: { title: string; p: ChargeParam | undefined; unit: string }) {
  if (!p) return null;
  const digits = 0;
  return (
    <tr>
      <td className="py-2 pr-3 font-medium">{title}</td>
      <td className="px-3 py-2 text-right tabular-nums">{p.desired === null ? "Standard" : num(p.desired, digits, unit)}</td>
      <td className="px-3 py-2 text-right tabular-nums">{num(p.sent, digits, unit)}</td>
      <td className="px-3 py-2 text-right font-semibold tabular-nums">{num(p.actual, digits, unit)}</td>
      <td className="py-2 pl-3">
        <StatusBadge status={p.status} />
        {p.error && <div className="mt-1 text-xs text-slate-500">{p.error}</div>}
      </td>
    </tr>
  );
}

function BatteryCare({ charge, settings, s, run }: { charge: ChargeControl | null; settings: Settings; s: Snapshot; run: Run }) {
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
            desc="Schreibt das Limit in den Embedded Controller (wie die BIOS-Einstellung) und setzt es nach Neustart/Netzteilwechsel erneut."
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
          <p className="mb-2 text-sm text-amber-700 dark:text-amber-400">Kein EC-Zugriff – Werte werden gesendet, sobald er da ist.</p>
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
        {charge?.fullOnceMs != null && (
          <p className="mt-2 text-xs text-slate-500">
            „Voll laden“ ausgelöst um {new Date(charge.fullOnceMs).toLocaleTimeString("de-DE")}.
          </p>
        )}
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

const eventStyle = {
  info: { icon: "ⓘ", color: "text-slate-500" },
  applied: { icon: "✓", color: "text-emerald-600 dark:text-emerald-400" },
  overridden: { icon: "↺", color: "text-orange-600 dark:text-orange-400" },
  warning: { icon: "⚠", color: "text-amber-600 dark:text-amber-400" },
  error: { icon: "✕", color: "text-red-600 dark:text-red-400" },
} as const;

function Events({ view }: { view: TunerView }) {
  const events = [...view.state.events].reverse();
  return (
    <Card title="Ereignisse des Tuners">
      {events.length === 0 && <div className="text-sm text-slate-400">Noch keine Ereignisse.</div>}
      <ul className="divide-y divide-slate-100 text-sm dark:divide-slate-800">
        {events.map((e, i) => (
          <li key={`${e.timestampMs}-${i}`} className="flex gap-3 py-2">
            <span className="w-20 shrink-0 tabular-nums text-slate-500">{new Date(e.timestampMs).toLocaleTimeString("de-DE")}</span>
            <span className={`w-4 shrink-0 ${eventStyle[e.kind].color}`} aria-label={e.kind}>
              {eventStyle[e.kind].icon}
            </span>
            <span>{e.message}</span>
          </li>
        ))}
      </ul>
    </Card>
  );
}
