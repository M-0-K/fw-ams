import type { Snapshot } from "../bindings/Snapshot";
import { num, tempColor } from "../format";

const MAX_TEMP = 110;

export default function ThermalPage({ s }: { s: Snapshot }) {
  const t = s.thermal;
  if (!t) return <div className="card text-slate-400">Keine Thermik-Daten verfügbar.</div>;

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <section className="card lg:col-span-2">
        <h2 className="mb-4 font-semibold">Temperaturen</h2>
        <div className="space-y-3">
          {t.sensors.map((sensor) => (
            <div key={sensor.name} className="grid grid-cols-[10rem_1fr_4rem] items-center gap-3">
              <div>
                <div className="text-sm font-medium">{sensor.label}</div>
                <div className="text-xs text-slate-500">{sensor.name}</div>
              </div>
              <div className="h-3 overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                <div
                  className={`h-full rounded-full ${tempColor(sensor.celsius)}`}
                  style={{ width: `${Math.min(100, ((sensor.celsius ?? 0) / MAX_TEMP) * 100)}%`, transition: "width 0.6s" }}
                />
              </div>
              <div className="text-right font-semibold tabular-nums">
                {sensor.error ?? num(sensor.celsius, 0, "°C")}
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="card">
        <h2 className="mb-4 font-semibold">Lüfter</h2>
        {t.fans.length === 0 && <div className="text-slate-400">Keine Lüfter gemeldet</div>}
        {t.fans.map((f) => (
          <div key={f.name} className="flex items-center justify-between py-1">
            <span>{f.name}</span>
            <span className="value">
              {f.stalled ? <span className="text-red-500">blockiert</span> : f.rpm !== null ? `${f.rpm} RPM` : "n/v"}
            </span>
          </div>
        ))}
      </section>

      <section className="card">
        <h2 className="mb-4 font-semibold">Drosselung</h2>
        {t.throttle ? (
          <div className="grid grid-cols-2 gap-3">
            {[
              ["Soft (Leistungslimit)", t.throttle.soft],
              ["Hart (PROCHOT#)", t.throttle.hard],
            ].map(([k, v]) => (
              <div
                key={String(k)}
                className={`rounded-lg p-3 text-center ${v ? "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-200" : "bg-emerald-50 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-200"}`}
              >
                <div className="text-xs">{k}</div>
                <div className="font-semibold">{v ? "aktiv" : "inaktiv"}</div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-slate-400">vom EC nicht gemeldet</div>
        )}
      </section>
    </div>
  );
}
