import { useEffect, useState } from "react";
import { getSetup, resetSystem } from "../api";
import type { ResetReport } from "../bindings/ResetReport";
import type { SetupInfo } from "../bindings/SetupInfo";
import type { StepResult } from "../bindings/StepResult";
import { Button, Notice } from "./ui";

const resultStyle: Record<StepResult, { icon: string; label: string; color: string }> = {
  done: { icon: "✓", label: "erledigt", color: "text-emerald-600 dark:text-emerald-400" },
  skipped: { icon: "–", label: "nichts zu tun", color: "text-slate-400" },
  failed: { icon: "✕", label: "fehlgeschlagen", color: "text-red-600 dark:text-red-400" },
};

/**
 * Explains what a reset undoes, asks whether to remove the helper service, runs it and shows
 * the report step by step. `factory` = all settings to defaults, otherwise only the extra
 * features are switched off.
 */
export default function ResetPanel({ factory, onClose }: { factory: boolean; onClose: () => void }) {
  const [setup, setSetup] = useState<SetupInfo | null>(null);
  const [removeService, setRemoveService] = useState(true);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ResetReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSetup().then(setSetup).catch(() => {});
  }, []);

  const serviceInstalled = setup?.serviceInstalled ?? false;
  const start = () => {
    setBusy(true);
    setError(null);
    resetSystem({ factory, removeService: removeService && serviceInstalled })
      .then(setReport)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };

  if (report) {
    const failed = report.steps.some((s) => s.result === "failed");
    return (
      <div className="space-y-3 py-3" role="status">
        <Notice tone={failed ? "warn" : "ok"}>
          {failed
            ? "Zurückgesetzt, aber nicht alles hat geklappt – Details unten."
            : factory
              ? "fw-ams ist auf Werkseinstellungen zurückgesetzt. Das System läuft wieder mit den Werten von vorher."
              : "Erweiterte Funktionen sind aus. Das System läuft wieder mit den Werten von vorher."}
        </Notice>
        <ul className="space-y-1.5 text-sm">
          {report.steps.map((s) => {
            const r = resultStyle[s.result];
            return (
              <li key={s.label} className="flex gap-2">
                <span className={`w-4 shrink-0 text-center ${r.color}`} aria-label={r.label}>
                  {r.icon}
                </span>
                <span>
                  <span className="font-medium">{s.label}</span>
                  {s.detail && <span className="text-slate-500"> – {s.detail}</span>}
                </span>
              </li>
            );
          })}
        </ul>
        <Button onClick={onClose}>Schließen</Button>
      </div>
    );
  }

  return (
    <div className="space-y-3 py-3">
      <div className="text-sm">
        <div className="mb-1 font-medium">Das wird rückgängig gemacht:</div>
        <ul className="list-inside list-disc space-y-0.5 text-slate-600 dark:text-slate-300">
          <li>CPU-Tuning: Profile leeren, Powerlimits, Curve Optimizer und Windows-Energieplan auf Standard</li>
          <li>Ladelimit: ursprünglicher Wert zurück in den Embedded Controller, Ladestrom ohne Limit</li>
          <li>Energiesparmodus beenden, vorherigen Windows-Energiemodus wiederherstellen</li>
          {factory ? (
            <>
              <li>Alle Einstellungen auf Standard, Autostart-Eintrag entfernen</li>
              <li>Gemerkte Expansion Cards löschen</li>
            </>
          ) : (
            <li className="list-none pt-1 text-slate-500">
              Die Überwachung (Akku, Ports, Thermik, Benachrichtigungen) läuft weiter. Einschalten geht jederzeit
              wieder.
            </li>
          )}
        </ul>
      </div>
      {serviceInstalled && (
        <label className="flex items-start gap-2 text-sm">
          <input
            type="checkbox"
            className="mt-0.5 accent-accent"
            checked={removeService}
            onChange={(e) => setRemoveService(e.target.checked)}
          />
          <span>
            Hilfsdienst deinstallieren und seine Profile löschen (eine Admin-Abfrage)
            <span className="block text-xs text-slate-500">
              Ohne Deinstallation bleibt der Dienst mit leeren Profilen installiert.
            </span>
          </span>
        </label>
      )}
      <p className="text-xs text-slate-500">
        Powerlimits und Curve Optimizer liegen nur im Arbeitsspeicher der CPU – spätestens ein Neustart setzt sie sicher
        zurück.
      </p>
      {error && <Notice tone="error">{error}</Notice>}
      <div className="flex flex-wrap gap-2">
        <Button variant="danger" onClick={start} disabled={busy}>
          {busy ? "Bitte warten…" : factory ? "Auf Werkseinstellungen zurücksetzen" : "Ausschalten und zurücksetzen"}
        </Button>
        <Button onClick={onClose} disabled={busy}>
          Abbrechen
        </Button>
      </div>
    </div>
  );
}
