import type { ReactNode } from "react";
import { openPawnioSite, tunerInstall, tunerUninstall } from "../../api";
import type { TunerView } from "../../bindings/TunerView";
import { Button, ConfirmButton, Notice } from "../../components/ui";
import type { Run } from "./shared";

function Step({ done, children }: { done: boolean; children: ReactNode }) {
  return (
    <li className="flex gap-2">
      <span className={done ? "text-emerald-600 dark:text-emerald-400" : "text-slate-400"} aria-hidden>
        {done ? "✓" : "○"}
      </span>
      <span>{children}</span>
    </li>
  );
}

/** Service state: one green line when ready, otherwise the steps to set it up */
export default function Setup({ view, busy, run }: { view: TunerView; busy: boolean; run: Run }) {
  const st = view.state;
  const setup = view.setup;
  if (st.availability === "ready" || st.availability === "mock") {
    return (
      <Notice tone="ok" className="flex flex-wrap items-center justify-between gap-2 py-2">
        <span>
          ✓{" "}
          {st.availability === "mock"
            ? "Simulierte Hardware (--mock)"
            : `Hilfsdienst ${st.serviceVersion ?? ""} aktiv, SMU-Zugriff über PawnIO`}
        </span>
        {st.availability === "ready" && (
          <ConfirmButton
            disabled={busy}
            question="Dienst entfernen? Der Windows-Energieplan wird wiederhergestellt, Powerlimits gelten bis zum Neustart."
            confirmLabel="Entfernen"
            onConfirm={() => run(() => tunerUninstall(), "Hilfsdienst entfernt")}
          >
            Dienst entfernen
          </ConfirmButton>
        )}
      </Notice>
    );
  }
  return (
    <Notice tone="warn" className="p-4">
      <div className="mb-2 font-semibold">Erweiterte CPU-Funktionen einrichten</div>
      <p className="mb-3">{st.message}</p>
      {setup.platformSupported && (
        <>
          <ol className="mb-3 space-y-1.5">
            <Step done={setup.pawnioInstalled}>
              PawnIO-Treiber installieren (signierter Nachfolger von WinRing0).{" "}
              {!setup.pawnioInstalled && (
                <button className="underline" onClick={() => run(openPawnioSite, "pawnio.eu geöffnet")}>
                  pawnio.eu öffnen
                </button>
              )}
            </Step>
            <Step done={setup.moduleFound}>
              {setup.moduleFound
                ? "RyzenSMU-Modul gefunden"
                : "RyzenSMU-Modul fehlt (Entwicklung: scripts/fetch-pawnio-module.ps1 ausführen)"}
            </Step>
            <Step done={setup.serviceRunning}>
              fw-ams-Hilfsdienst installieren (eine Admin-Abfrage). Er nimmt nur geprüfte, begrenzte Werte an.
              {!setup.serviceExeFound && " – fw-ams-service.exe nicht gefunden (cargo build -p fw-ams-service)."}
            </Step>
          </ol>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="primary" disabled={busy || !setup.serviceExeFound} onClick={() => run(tunerInstall, "Hilfsdienst installiert")}>
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
    </Notice>
  );
}
