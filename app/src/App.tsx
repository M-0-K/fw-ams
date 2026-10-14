import { useState } from "react";
import { useSnapshot } from "./api";
import Overview from "./pages/Overview";
import Ports from "./pages/Ports";
import ThermalPage from "./pages/Thermal";
import SystemPage from "./pages/System";
import SettingsPage from "./pages/Settings";

const pages = [
  { id: "overview", label: "Übersicht", icon: "◐" },
  { id: "ports", label: "Ports & Karten", icon: "⇆" },
  { id: "thermal", label: "Thermik", icon: "≋" },
  { id: "system", label: "System", icon: "ⓘ" },
  { id: "settings", label: "Einstellungen", icon: "⚙" },
] as const;

type PageId = (typeof pages)[number]["id"];

export default function App() {
  const [page, setPage] = useState<PageId>("overview");
  const snapshot = useSnapshot();

  return (
    <div className="flex h-screen">
      <nav className="flex w-52 shrink-0 flex-col gap-1 border-r border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
        <div className="mb-4 flex items-center gap-2 px-2 pt-2">
          <span className="h-3 w-3 rounded-full bg-accent" />
          <span className="text-lg font-bold">fw-ams</span>
        </div>
        {pages.map((p) => (
          <button
            key={p.id}
            onClick={() => setPage(p.id)}
            className={`flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm font-medium transition ${
              page === p.id
                ? "bg-accent/10 text-accent"
                : "text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            <span className="w-4 text-center">{p.icon}</span>
            {p.label}
          </button>
        ))}
        <div className="mt-auto px-2 text-xs text-slate-400">
          {snapshot?.access.ec === "mock" && <div className="mb-1 font-semibold text-amber-500">Mock-Daten</div>}
          {snapshot ? `Stand ${new Date(snapshot.timestampMs).toLocaleTimeString("de-DE")}` : "Warte auf Daten…"}
        </div>
      </nav>

      <main className="flex-1 overflow-y-auto p-6">
        {!snapshot ? (
          <div className="flex h-full items-center justify-center text-slate-400">Lese Hardware…</div>
        ) : (
          <>
            {snapshot.access.hint && snapshot.access.ec !== "mock" && (
              <div className="mb-4 rounded-xl border border-amber-300 bg-amber-50 p-4 text-sm text-amber-900 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200">
                {snapshot.access.hint}
              </div>
            )}
            {page === "overview" && <Overview s={snapshot} onOpenPorts={() => setPage("ports")} />}
            {page === "ports" && <Ports s={snapshot} />}
            {page === "thermal" && <ThermalPage s={snapshot} />}
            {page === "system" && <SystemPage s={snapshot} />}
          </>
        )}
        {page === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}
