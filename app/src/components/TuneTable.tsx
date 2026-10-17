import { useEffect, useState } from "react";
import type { ParamId } from "../bindings/ParamId";
import type { ParamSpec } from "../bindings/ParamSpec";
import type { ParamState } from "../bindings/ParamState";
import type { ParamStatus } from "../bindings/ParamStatus";
import { boostLabels, num, paramMeta, paramValue, statusStyle } from "../format";

export function StatusBadge({ status, restored = 0 }: { status: ParamStatus; restored?: number }) {
  const s = statusStyle[status];
  return (
    <span className="inline-flex flex-wrap items-center gap-1">
      <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${s.color}`}>
        <span aria-hidden>{s.icon}</span>
        {s.label}
      </span>
      {restored > 0 && (
        <span
          className="rounded-full bg-orange-100 px-2 py-0.5 text-xs text-orange-800 dark:bg-orange-950 dark:text-orange-300"
          title="So oft hat die Firmware den Wert zurückgesetzt und fw-ams ihn wiederhergestellt"
        >
          ↺ {restored}× wiederhergestellt
        </span>
      )}
    </span>
  );
}

const show = (v: number | null) => (v === null ? "" : String(v).replace(".", ","));

/** Empty = firmware default. Commits on blur/Enter; invalid input reverts. */
function ValueInput({
  id,
  spec,
  value,
  onChange,
  disabled,
}: {
  id: ParamId;
  spec: ParamSpec;
  value: number | null;
  onChange: (v: number | null) => void;
  disabled?: boolean;
}) {
  const [text, setText] = useState(show(value));
  useEffect(() => setText(show(value)), [value]);

  if (id === "boost") {
    return (
      <select
        value={value === null ? "" : String(value)}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
        aria-label={paramMeta[id].label}
        className="w-40 rounded-md border border-slate-300 bg-transparent px-2 py-1 text-sm dark:border-slate-600 dark:bg-slate-900"
      >
        <option value="">Standard</option>
        {boostLabels.map((l, i) => (
          <option key={l} value={i}>
            {l}
          </option>
        ))}
      </select>
    );
  }

  const commit = () => {
    if (text.trim() === "") return onChange(null);
    const parsed = Number(text.replace(",", "."));
    if (!Number.isFinite(parsed)) return setText(show(value));
    const step = spec.step || 1;
    const v = Math.min(spec.max, Math.max(spec.min, Math.round(parsed / step) * step));
    setText(show(v));
    if (v !== value) onChange(v);
  };
  const warn =
    value !== null &&
    ((spec.warnAbove !== null && value > spec.warnAbove) || (spec.warnBelow !== null && value < spec.warnBelow));

  return (
    <span className="flex items-center gap-1.5">
      <input
        inputMode="decimal"
        placeholder="Standard"
        value={text}
        disabled={disabled}
        aria-label={paramMeta[id].label}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        className={`w-24 rounded-md border bg-transparent px-2 py-1 text-right text-sm tabular-nums placeholder:text-slate-400 focus:border-accent focus:outline-none disabled:opacity-40 ${
          warn ? "border-orange-400 dark:border-orange-500" : "border-slate-300 dark:border-slate-600"
        }`}
      />
      <span className="w-8 text-xs text-slate-500">{paramMeta[id].unit}</span>
    </span>
  );
}

/**
 * Transparency table: firmware default, value in the profile being edited, value sent,
 * value the hardware reports, and the resulting status.
 */
export default function TuneTable({
  ids,
  specs,
  params,
  draft,
  saved,
  editingActive,
  onChange,
}: {
  ids: ParamId[];
  specs: ParamSpec[];
  params: ParamState[];
  /** Values of the profile being edited (unsaved) */
  draft: (id: ParamId) => number | null;
  /** Values of the same profile as stored in the service */
  saved: (id: ParamId) => number | null;
  /** The edited profile is the one currently active (sent/actual refer to it) */
  editingActive: boolean;
  onChange: (id: ParamId, v: number | null) => void;
}) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-slate-200 text-left text-xs uppercase tracking-wide text-slate-500 dark:border-slate-800">
            <th className="py-2 pr-3 font-medium">Parameter</th>
            <th className="px-3 py-2 text-right font-medium">Standard</th>
            <th className="px-3 py-2 font-medium">Eingestellt</th>
            <th className="px-3 py-2 text-right font-medium">Gesendet</th>
            <th className="px-3 py-2 text-right font-medium">Ist (Hardware)</th>
            <th className="py-2 pl-3 font-medium">Status</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
          {ids.map((id) => {
            const spec = specs.find((s) => s.id === id);
            const p = params.find((x) => x.id === id);
            if (!spec || !p) return null;
            const changed = draft(id) !== saved(id);
            const meta = paramMeta[id];
            return (
              <tr key={id} className={changed ? "bg-amber-50 dark:bg-amber-950/40" : ""}>
                <td className="py-2 pr-3 align-top">
                  <div className="font-medium">{meta.label}</div>
                  <div className="max-w-xs text-xs text-slate-500">{meta.desc}</div>
                  <div className="text-xs text-slate-400">
                    Bereich {paramValue(id, spec.min)} … {paramValue(id, spec.max)}
                    {spec.warnAbove !== null && id !== "coAll" && ` · Warnung über ${paramValue(id, spec.warnAbove)}`}
                  </div>
                </td>
                <td className="px-3 py-2 text-right align-top tabular-nums text-slate-500">{paramValue(id, p.default)}</td>
                <td className="px-3 py-2 align-top">
                  <ValueInput
                    id={id}
                    spec={spec}
                    value={draft(id)}
                    disabled={!spec.supported}
                    onChange={(v) => onChange(id, v)}
                  />
                  {changed && <div className="mt-1 text-xs text-amber-700 dark:text-amber-400">nicht übernommen</div>}
                </td>
                <td className="px-3 py-2 text-right align-top tabular-nums">
                  {editingActive ? paramValue(id, p.sent) : <span className="text-slate-400">–</span>}
                </td>
                <td className="px-3 py-2 text-right align-top font-semibold tabular-nums">
                  {spec.readable ? paramValue(id, p.actual) : <span className="font-normal text-slate-400">nicht lesbar</span>}
                </td>
                <td className="py-2 pl-3 align-top">
                  {editingActive || p.status === "unsupported" ? (
                    <StatusBadge status={p.status} restored={p.restored} />
                  ) : (
                    <span className="text-xs text-slate-400">Profil gerade nicht aktiv</span>
                  )}
                  {p.error && editingActive && <div className="mt-1 max-w-56 text-xs text-slate-500">{p.error}</div>}
                  {p.changedMs !== null && editingActive && (
                    <div className="mt-0.5 text-xs text-slate-400">
                      gesendet {new Date(p.changedMs).toLocaleTimeString("de-DE")}
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="mt-2 text-xs text-slate-500">
        Leeres Feld = Standard der Firmware. Werte werden auf den erlaubten Bereich geklemmt ({num(null)} = nicht gemeldet).
      </p>
    </div>
  );
}
