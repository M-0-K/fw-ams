import { useEffect, useRef, useState } from "react";
import type { HistorySample } from "../bindings/HistorySample";
import { num } from "../format";

export type Series = {
  label: string;
  /** CSS color, use the --series-N variables in fixed order */
  color: string;
  get: (s: HistorySample) => number | null;
};

/** Limit drawn as a thin dashed reference line (it may change over time) */
export type Reference = {
  label: string;
  get: (s: HistorySample) => number | null;
};

const PAD = { top: 8, right: 12, bottom: 20, left: 40 };

function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(600);
  useEffect(() => {
    if (!ref.current) return;
    const ro = new ResizeObserver(([e]) => setWidth(Math.max(200, e.contentRect.width)));
    ro.observe(ref.current);
    return () => ro.disconnect();
  }, []);
  return [ref, width] as const;
}

function niceMax(v: number): number {
  if (v <= 0) return 1;
  const p = 10 ** Math.floor(Math.log10(v));
  const n = v / p;
  return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 2.5 ? 2.5 : n <= 5 ? 5 : 10) * p;
}

/** Path with gaps where values are missing */
function path(points: ([number, number] | null)[]): string {
  let d = "";
  let pen = false;
  for (const p of points) {
    if (!p) {
      pen = false;
      continue;
    }
    d += `${pen ? "L" : "M"}${p[0].toFixed(1)},${p[1].toFixed(1)}`;
    pen = true;
  }
  return d;
}

/**
 * Time series chart: one y-axis, 2px lines, recessive grid, legend with current values,
 * crosshair + tooltip on hover.
 */
export default function LineChart({
  title,
  unit,
  digits = 1,
  samples,
  series,
  refs = [],
  height = 170,
  yMax,
}: {
  title: string;
  unit: string;
  digits?: number;
  samples: HistorySample[];
  series: Series[];
  refs?: Reference[];
  height?: number;
  /** Fixed upper bound (e.g. 100 % usage); otherwise derived from the data */
  yMax?: number;
}) {
  const [box, width] = useWidth();
  const [hover, setHover] = useState<number | null>(null);
  const plotW = width - PAD.left - PAD.right;
  const plotH = height - PAD.top - PAD.bottom;

  const n = samples.length;
  const t0 = n ? samples[0].t : 0;
  const t1 = n ? samples[n - 1].t : 1;
  const span = Math.max(t1 - t0, 60_000);
  const values = samples.flatMap((s) => [...series.map((x) => x.get(s)), ...refs.map((r) => r.get(s))]);
  const top = yMax ?? niceMax(Math.max(1, ...values.filter((v): v is number => v !== null && Number.isFinite(v))) * 1.05);
  const x = (t: number) => PAD.left + ((t - (t1 - span)) / span) * plotW;
  const y = (v: number) => PAD.top + plotH - (Math.min(v, top) / top) * plotH;
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((f) => f * top);
  const last = n ? samples[n - 1] : null;
  const shown = hover !== null && samples[hover] ? samples[hover] : null;

  const onMove = (e: React.MouseEvent<SVGRectElement>) => {
    if (!n) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const t = t1 - span + ((e.clientX - rect.left) / rect.width) * span;
    let best = 0;
    for (let i = 1; i < n; i++) if (Math.abs(samples[i].t - t) < Math.abs(samples[best].t - t)) best = i;
    setHover(best);
  };

  const fmt = (v: number | null) => num(v, digits, unit);

  return (
    <figure className="min-w-0">
      <figcaption className="mb-2 flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
        <span className="text-sm font-semibold">{title}</span>
        <span className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-slate-600 dark:text-slate-300">
          {series.map((s) => (
            <span key={s.label} className="flex items-center gap-1.5">
              <span className="inline-block h-0.5 w-4 rounded" style={{ background: s.color }} />
              {s.label}
              <span className="font-semibold tabular-nums text-slate-900 dark:text-slate-100">{fmt(last && s.get(last))}</span>
            </span>
          ))}
          {refs.map((r) => (
            <span key={r.label} className="flex items-center gap-1.5">
              <span className="inline-block w-4 border-t border-dashed" style={{ borderColor: "var(--chart-ref)" }} />
              {r.label}
              <span className="tabular-nums">{fmt(last && r.get(last))}</span>
            </span>
          ))}
        </span>
      </figcaption>
      <div ref={box} className="relative">
        <svg width={width} height={height} role="img" aria-label={title} className="block">
          {ticks.map((v) => (
            <g key={v}>
              <line x1={PAD.left} x2={width - PAD.right} y1={y(v)} y2={y(v)} stroke="var(--chart-grid)" strokeWidth={1} />
              <text x={PAD.left - 6} y={y(v) + 3} textAnchor="end" className="fill-slate-500 text-[10px] tabular-nums">
                {num(v, top < 10 ? 1 : 0)}
              </text>
            </g>
          ))}
          <text x={PAD.left} y={height - 4} className="fill-slate-500 text-[10px]">
            −{Math.round(span / 60_000)} min
          </text>
          <text x={width - PAD.right} y={height - 4} textAnchor="end" className="fill-slate-500 text-[10px]">
            jetzt
          </text>
          {refs.map((r) => (
            <path
              key={r.label}
              d={path(samples.map((s) => (r.get(s) !== null ? [x(s.t), y(r.get(s)!)] : null)))}
              fill="none"
              stroke="var(--chart-ref)"
              strokeWidth={1}
              strokeDasharray="4 3"
            />
          ))}
          {series.map((s) => (
            <path
              key={s.label}
              d={path(samples.map((p) => (s.get(p) !== null ? [x(p.t), y(s.get(p)!)] : null)))}
              fill="none"
              stroke={s.color}
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          ))}
          {shown && (
            <g>
              <line x1={x(shown.t)} x2={x(shown.t)} y1={PAD.top} y2={PAD.top + plotH} stroke="var(--chart-ref)" strokeWidth={1} />
              {series.map((s) => {
                const v = s.get(shown);
                return v === null ? null : (
                  <circle
                    key={s.label}
                    cx={x(shown.t)}
                    cy={y(v)}
                    r={4}
                    fill={s.color}
                    className="stroke-white dark:stroke-slate-900"
                    strokeWidth={2}
                  />
                );
              })}
            </g>
          )}
          <rect
            x={PAD.left}
            y={PAD.top}
            width={Math.max(0, plotW)}
            height={plotH}
            fill="transparent"
            onMouseMove={onMove}
            onMouseLeave={() => setHover(null)}
          />
          {n === 0 && (
            <text x={PAD.left + plotW / 2} y={PAD.top + plotH / 2} textAnchor="middle" className="fill-slate-400 text-xs">
              Sammle Daten…
            </text>
          )}
        </svg>
        {shown && (
          <div
            className="pointer-events-none absolute top-1 z-10 rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs shadow-md dark:border-slate-700 dark:bg-slate-800"
            style={x(shown.t) > width / 2 ? { right: width - x(shown.t) + 8 } : { left: x(shown.t) + 8 }}
          >
            <div className="mb-1 text-slate-500">{new Date(shown.t).toLocaleTimeString("de-DE")}</div>
            {series.map((s) => (
              <div key={s.label} className="flex items-center justify-between gap-4">
                <span className="flex items-center gap-1.5">
                  <span className="inline-block h-2 w-2 rounded-full" style={{ background: s.color }} />
                  {s.label}
                </span>
                <span className="font-semibold tabular-nums">{fmt(s.get(shown))}</span>
              </div>
            ))}
            {refs.map((r) => (
              <div key={r.label} className="flex justify-between gap-4 text-slate-500">
                <span>{r.label}</span>
                <span className="tabular-nums">{fmt(r.get(shown))}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </figure>
  );
}
