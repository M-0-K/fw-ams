import type { ChargeState } from "../bindings/ChargeState";

const RADIUS = 52;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export default function BatteryRing({
  percent,
  state,
  limit,
}: {
  percent: number;
  state: ChargeState;
  limit: number | null;
}) {
  const clamped = Math.max(0, Math.min(100, percent));
  const color =
    state === "charging" ? "#22c55e" : clamped <= 15 ? "#ef4444" : clamped <= 30 ? "#f59e0b" : "#f97316";
  const limitAngle = limit !== null && limit < 100 ? (limit / 100) * 360 - 90 : null;

  return (
    <svg viewBox="0 0 120 120" className="h-44 w-44">
      <circle cx="60" cy="60" r={RADIUS} fill="none" strokeWidth="10" className="stroke-slate-200 dark:stroke-slate-800" />
      <circle
        cx="60"
        cy="60"
        r={RADIUS}
        fill="none"
        stroke={color}
        strokeWidth="10"
        strokeLinecap="round"
        strokeDasharray={CIRCUMFERENCE}
        strokeDashoffset={CIRCUMFERENCE * (1 - clamped / 100)}
        transform="rotate(-90 60 60)"
        style={{ transition: "stroke-dashoffset 0.6s ease" }}
      />
      {limitAngle !== null && (
        <line
          x1={60 + 44 * Math.cos((limitAngle * Math.PI) / 180)}
          y1={60 + 44 * Math.sin((limitAngle * Math.PI) / 180)}
          x2={60 + 60 * Math.cos((limitAngle * Math.PI) / 180)}
          y2={60 + 60 * Math.sin((limitAngle * Math.PI) / 180)}
          strokeWidth="2.5"
          className="stroke-slate-500 dark:stroke-slate-300"
        >
          <title>Ladelimit {limit} %</title>
        </line>
      )}
      <text x="60" y="58" textAnchor="middle" className="fill-current text-[22px] font-bold">
        {Math.round(clamped)}%
      </text>
      <text x="60" y="76" textAnchor="middle" className="fill-slate-500 text-[9px]">
        {state === "charging" ? "⚡ lädt" : state === "discharging" ? "Akku" : state === "idle" ? "am Netz" : ""}
      </text>
    </svg>
  );
}
