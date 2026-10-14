import type { Snapshot } from "../bindings/Snapshot";
import { modelLabel } from "../format";

export default function SystemPage({ s }: { s: Snapshot }) {
  const rows: [string, string][] = [
    ["Modell", modelLabel[s.system.model]],
    ["Produkt (SMBIOS)", s.system.product ?? "n/v"],
    ["BIOS", s.system.biosVersion ?? "n/v"],
    ["EC-Firmware", s.system.ecVersion ?? "n/v"],
    ["PD-Controller", s.system.pdVersions.length ? s.system.pdVersions.join(" / ") : "n/v"],
    ["Framework-Gerät", s.access.isFramework ? "ja" : "nein"],
    [
      "EC-Zugriff",
      s.access.ec === "ok" ? "verbunden" : s.access.ec === "mock" ? "Mock-Daten" : "nicht verfügbar",
    ],
    ["Akku-Datenquelle", s.battery?.source ?? "n/v"],
    ["Thermik-Datenquelle", s.thermal?.source ?? "n/v"],
  ];
  return (
    <div className="space-y-4">
      <section className="card">
        <h2 className="mb-4 font-semibold">System</h2>
        <table className="w-full text-sm">
          <tbody>
            {rows.map(([k, v]) => (
              <tr key={k} className="border-b border-slate-100 last:border-0 dark:border-slate-800">
                <td className="py-2 text-slate-500">{k}</td>
                <td className="py-2 font-medium select-text">{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
      {s.access.hint && (
        <section className="card">
          <h2 className="mb-2 font-semibold">Hinweis</h2>
          <p className="text-sm">{s.access.hint}</p>
        </section>
      )}
    </div>
  );
}
