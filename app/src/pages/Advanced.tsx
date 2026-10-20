import { useState } from "react";
import type { ProfileSlot } from "../bindings/ProfileSlot";
import type { Snapshot } from "../bindings/Snapshot";
import type { TunerConfig } from "../bindings/TunerConfig";
import type { TunerView } from "../bindings/TunerView";
import { useChargeControl, useSettings, useTuner } from "../api";
import Stat from "../components/Stat";
import { Toast, useFlash } from "../components/ui";
import BatteryCare from "./advanced/BatteryCare";
import Events from "./advanced/Events";
import Live from "./advanced/Live";
import Setup from "./advanced/Setup";
import { modeLabel, slotLabel, type Run } from "./advanced/shared";
import Tuning from "./advanced/Tuning";

const tabs = [
  { id: "live", label: "Live-Analyse" },
  { id: "power", label: "Leistung & Takt" },
  { id: "voltage", label: "Spannung" },
  { id: "battery", label: "Akkupflege" },
  { id: "events", label: "Ereignisse" },
] as const;
type Tab = (typeof tabs)[number]["id"];

export default function AdvancedPage({ s }: { s: Snapshot }) {
  const { view, history } = useTuner();
  const charge = useChargeControl();
  const settings = useSettings();
  const [tab, setTab] = useState<Tab>("live");
  const [draft, setDraft] = useState<TunerConfig | null>(null);
  const [slot, setSlot] = useState<ProfileSlot | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, flash] = useFlash();

  if (!view) return <div className="card text-slate-400">Verbinde mit dem Tuner…</div>;
  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(view.state.config);

  const run: Run = async (what, ok) => {
    setBusy(true);
    try {
      await what();
      flash(ok);
      return true;
    } catch (e) {
      flash(String(e), true);
      return false;
    } finally {
      setBusy(false);
    }
  };

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
            {dirty && (t.id === "power" || t.id === "voltage") && (
              <span className="ml-1 text-amber-500" title="Nicht übernommene Änderungen">
                ●
              </span>
            )}
          </button>
        ))}
        {busy && <span className="ml-auto text-sm text-slate-500">Bitte warten…</span>}
      </div>

      {tab === "live" && <Live view={view} history={history} />}
      {(tab === "power" || tab === "voltage") && (
        <Tuning
          tab={tab}
          view={view}
          draft={{ value: draft, set: setDraft, slot, setSlot }}
          busy={busy}
          run={run}
        />
      )}
      {tab === "battery" && settings && <BatteryCare charge={charge} settings={settings} s={s} run={run} />}
      {tab === "events" && <Events view={view} />}
      <Toast msg={msg} />
    </div>
  );
}

function Header({ view, s }: { view: TunerView; s: Snapshot }) {
  const st = view.state;
  const cpu = st.cpu;
  return (
    <div className="card grid grid-cols-2 gap-4 md:grid-cols-5">
      <Stat
        label="CPU"
        value={<span className="text-base">{cpu?.name ?? "n/v"}</span>}
        sub={cpu ? `${cpu.family} · ${cpu.cores} Kerne / ${cpu.threads} Threads` : undefined}
      />
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
