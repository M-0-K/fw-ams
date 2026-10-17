import type { Snapshot } from "../bindings/Snapshot";
import BatteryRing from "../components/BatteryRing";
import LaptopDiagram from "../components/LaptopDiagram";
import Stat from "../components/Stat";
import { chargeStateLabel, duration, num, signed, tempColor } from "../format";

export default function Overview({ s, onOpenPorts }: { s: Snapshot; onOpenPorts: (slot?: number) => void }) {
  const b = s.battery;
  const chargerPort = s.ports.find((p) => p.index === s.charger.port);
  const hottest = s.thermal?.sensors
    .filter((x) => x.celsius !== null)
    .sort((a, c) => (c.celsius ?? 0) - (a.celsius ?? 0))[0];
  const fan = s.thermal?.fans[0];

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
      <section className="card md:col-span-2">
        <h2 className="mb-4 font-semibold">Akku</h2>
        {b ? (
          <div className="flex flex-wrap items-center gap-8">
            <BatteryRing percent={b.socPercent} state={b.state} limit={b.chargeLimitPercent} />
            <div className="grid flex-1 grid-cols-2 gap-x-8 gap-y-4 md:grid-cols-3">
              <Stat label="Status" value={chargeStateLabel[b.state]} sub={b.critical ? "kritisch" : undefined} />
              <Stat label="Leistung" value={signed(b.powerW, 1, "W")} sub={`${num(b.voltageV, 2, "V")} · ${signed(b.currentA, 2, "A")}`} />
              <Stat
                label={b.state === "charging" ? "Bis Ladelimit" : "Restlaufzeit"}
                value={duration(b.minutesRemaining)}
              />
              <Stat label="Gesundheit" value={num(b.healthPercent, 1, "%")} sub={`${b.fullMah ?? "?"} / ${b.designMah ?? "?"} mAh`} />
              <Stat label="Ladezyklen" value={b.cycleCount ?? "n/v"} />
              <Stat label="Ladelimit" value={b.chargeLimitPercent !== null ? `${b.chargeLimitPercent} %` : "n/v"} />
              <Stat label="Temperatur" value={num(b.temperatureC, 1, "°C")} />
              <Stat label="Akku" value={b.model ?? "n/v"} sub={b.manufacturer ?? undefined} />
            </div>
          </div>
        ) : (
          <div className="text-slate-400">Kein Akku gefunden</div>
        )}
      </section>

      <section className="card">
        <h2 className="mb-4 font-semibold">Ladegerät</h2>
        <div className="grid grid-cols-2 gap-4">
          <Stat label="Netzteil" value={s.charger.acPresent ? "angeschlossen" : "—"} />
          <Stat label="Port" value={chargerPort?.location.label ?? "—"} />
          <Stat
            label="Vertrag"
            value={num(s.charger.contractW, 0, "W")}
            sub={s.charger.voltageV !== null ? `${num(s.charger.voltageV, 1, "V")} × ${num(s.charger.currentA, 2, "A")}` : undefined}
          />
          <Stat label="EPR" value={s.charger.epr ? "aktiv" : "nein"} />
        </div>
      </section>

      <section className="card">
        <h2 className="mb-4 font-semibold">Thermik</h2>
        <div className="grid grid-cols-2 gap-4">
          <Stat
            label={hottest ? `Max. (${hottest.label})` : "Temperatur"}
            value={
              <span className="flex items-center gap-2">
                <span className={`h-2.5 w-2.5 rounded-full ${tempColor(hottest?.celsius ?? null)}`} />
                {num(hottest?.celsius ?? null, 0, "°C")}
              </span>
            }
          />
          <Stat label={fan?.name ?? "Lüfter"} value={fan?.rpm !== undefined && fan?.rpm !== null ? `${fan.rpm} RPM` : "n/v"} />
          <Stat
            label="Drosselung"
            value={
              s.thermal?.throttle
                ? s.thermal.throttle.hard
                  ? "hart"
                  : s.thermal.throttle.soft
                    ? "aktiv"
                    : "nein"
                : "n/v"
            }
          />
        </div>
      </section>

      <section className="card md:col-span-2">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-semibold">Expansion Cards</h2>
          <button onClick={() => onOpenPorts()} className="text-sm text-accent hover:underline">
            Details →
          </button>
        </div>
        <LaptopDiagram slots={s.slots} compact onSelect={onOpenPorts} />
      </section>
    </div>
  );
}
