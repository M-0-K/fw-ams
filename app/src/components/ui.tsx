import { useEffect, useState, type ReactNode } from "react";

export function Card({
  title,
  children,
  actions,
  className = "",
}: {
  title: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <section className={`card ${className}`}>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-semibold">{title}</h2>
        {actions}
      </div>
      {children}
    </section>
  );
}

type Variant = "default" | "primary" | "danger";

const variants: Record<Variant, string> = {
  default: "border border-slate-300 hover:bg-slate-100 dark:border-slate-600 dark:hover:bg-slate-800",
  primary: "bg-accent text-white hover:brightness-110",
  danger: "bg-red-600 text-white hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600",
};

export function Button({
  children,
  onClick,
  variant = "default",
  disabled,
  title,
}: {
  children: ReactNode;
  onClick: () => void;
  variant?: Variant;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`rounded-lg px-3 py-1.5 text-sm font-medium transition focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40 ${variants[variant]}`}
    >
      {children}
    </button>
  );
}

/** Two-step button for actions that change the system: first click asks, second confirms */
export function ConfirmButton({
  children,
  question,
  confirmLabel = "Ja",
  onConfirm,
  disabled,
  variant = "default",
}: {
  children: ReactNode;
  question: ReactNode;
  confirmLabel?: string;
  onConfirm: () => void;
  disabled?: boolean;
  variant?: Variant;
}) {
  const [asking, setAsking] = useState(false);
  // Do not stay armed forever
  useEffect(() => {
    if (!asking) return;
    const t = window.setTimeout(() => setAsking(false), 8000);
    return () => window.clearTimeout(t);
  }, [asking]);

  if (!asking) {
    return (
      <Button variant={variant} disabled={disabled} onClick={() => setAsking(true)}>
        {children}
      </Button>
    );
  }
  return (
    <span className="inline-flex flex-wrap items-center gap-2" role="group">
      <span className="text-sm">{question}</span>
      <Button
        variant={variant === "default" ? "primary" : variant}
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
      >
        {confirmLabel}
      </Button>
      <Button onClick={() => setAsking(false)}>Abbrechen</Button>
    </span>
  );
}

type Tone = "info" | "ok" | "warn" | "error";

const tones: Record<Tone, string> = {
  info: "border-sky-200 bg-sky-50 text-sky-900 dark:border-sky-900 dark:bg-sky-950 dark:text-sky-100",
  ok: "border-emerald-300 bg-emerald-50 text-emerald-900 dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-200",
  warn: "border-amber-300 bg-amber-50 text-amber-950 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-100",
  error: "border-red-300 bg-red-50 text-red-900 dark:border-red-800 dark:bg-red-950 dark:text-red-200",
};

export function Notice({ tone = "info", children, className = "" }: { tone?: Tone; children: ReactNode; className?: string }) {
  return <div className={`rounded-xl border p-3 text-sm ${tones[tone]} ${className}`}>{children}</div>;
}

/** Small status message in the bottom right corner, e.g. "Gespeichert ✓" */
export function Toast({ msg }: { msg: { text: string; error?: boolean } | null }) {
  if (!msg) return null;
  return (
    <div
      role="status"
      className={`fixed bottom-4 right-4 z-20 max-w-sm rounded-lg px-4 py-2 text-sm shadow-lg ${
        msg.error ? "bg-red-600 text-white" : "bg-slate-800 text-white dark:bg-slate-700"
      }`}
    >
      {msg.text}
    </div>
  );
}

/** Status message that clears itself after a few seconds */
export function useFlash(): [{ text: string; error?: boolean } | null, (text: string, error?: boolean) => void] {
  const [msg, setMsg] = useState<{ text: string; error?: boolean } | null>(null);
  useEffect(() => {
    if (!msg) return;
    const t = window.setTimeout(() => setMsg(null), msg.error ? 8000 : 3000);
    return () => window.clearTimeout(t);
  }, [msg]);
  return [msg, (text, error) => setMsg({ text, error })];
}
