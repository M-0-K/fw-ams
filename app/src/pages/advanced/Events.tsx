import type { TunerView } from "../../bindings/TunerView";
import { Card } from "../../components/ui";

const eventStyle = {
  info: { icon: "ⓘ", color: "text-slate-500" },
  applied: { icon: "✓", color: "text-emerald-600 dark:text-emerald-400" },
  overridden: { icon: "↺", color: "text-orange-600 dark:text-orange-400" },
  warning: { icon: "⚠", color: "text-amber-600 dark:text-amber-400" },
  error: { icon: "✕", color: "text-red-600 dark:text-red-400" },
} as const;

export default function Events({ view }: { view: TunerView }) {
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
