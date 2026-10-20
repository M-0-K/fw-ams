import { useEffect, useRef, useState, type ReactNode } from "react";

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full transition disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-accent ${
        checked ? "bg-accent" : "bg-slate-300 dark:bg-slate-600"
      }`}
    >
      <span
        className={`absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-5" : ""
        }`}
      />
    </button>
  );
}

const show = (v: number) => String(v).replace(".", ",");

/** Number field that only commits valid values: empty or invalid input falls back to the old value */
export function NumberInput({
  value,
  min,
  max,
  unit,
  disabled,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  disabled?: boolean;
  onCommit: (v: number) => void;
}) {
  const [text, setText] = useState(show(value));
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    setText(show(value));
  }, [value]);

  const commit = (raw: string) => {
    window.clearTimeout(timer.current);
    const parsed = Number(raw.replace(",", "."));
    if (raw.trim() === "" || !Number.isFinite(parsed)) {
      setText(show(value));
      return;
    }
    const clamped = Math.min(max, Math.max(min, parsed));
    setText(show(clamped));
    if (clamped !== value) onCommit(clamped);
  };

  return (
    <span className={`flex items-center gap-1.5 text-sm ${disabled ? "opacity-40" : ""}`}>
      <input
        inputMode="decimal"
        value={text}
        disabled={disabled}
        aria-label={`Wert in ${unit}`}
        onChange={(e) => {
          setText(e.target.value);
          window.clearTimeout(timer.current);
          const next = e.target.value;
          timer.current = window.setTimeout(() => commit(next), 800);
        }}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        className="w-16 rounded-md border border-slate-300 bg-transparent px-2 py-1 text-right tabular-nums focus:border-accent focus:outline-none dark:border-slate-600"
      />
      <span className="w-6 text-slate-500">{unit}</span>
    </span>
  );
}

export function Row({ title, desc, children }: { title: string; desc?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {desc && <div className="text-xs text-slate-500">{desc}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-4">{children}</div>
    </div>
  );
}
