import type { HistorySample } from "../../bindings/HistorySample";
import type { SmuTelemetry } from "../../bindings/SmuTelemetry";
import type { TunerView } from "../../bindings/TunerView";
import LineChart from "../../components/LineChart";
import PowerBreakdown from "../../components/PowerBreakdown";
import Stat from "../../components/Stat";
import { Card } from "../../components/ui";
import { num } from "../../format";

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

export default function Live({ view, history }: { view: TunerView; history: HistorySample[] }) {
  const t = view.state.telemetry;
  const os = view.os;
  const last = history.length ? history[history.length - 1] : null;
  const limits = limiters(t);

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
          value={<span className="text-base">{!t ? "n/v" : limits.length ? limits.join(", ") : "nichts"}</span>}
          sub={t ? "Ist-Wert ≥ 97 % des Limits" : "braucht SMU-Zugriff"}
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
  return (
    <Card title="Threads (Windows)">
      {os.threadMhz.length === 0 ? (
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
