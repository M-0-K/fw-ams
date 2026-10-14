import type { Slot } from "../bindings/Slot";
import { cardStyle } from "../format";

/** Top view of the laptop with the expansion card slots on both sides. */
export default function LaptopDiagram({
  slots,
  selected,
  onSelect,
  compact = false,
}: {
  slots: Slot[];
  selected?: number;
  onSelect?: (slot: number) => void;
  compact?: boolean;
}) {
  const order = { back: 0, middle: 1, front: 2 } as const;
  const side = (s: "left" | "right") =>
    slots
      .filter((x) => x.location.side === s)
      .sort((a, b) => order[a.location.position] - order[b.location.position]);

  const column = (list: Slot[]) => (
    <div className="flex flex-col justify-center gap-3">
      {list.map((slot) => {
        const style = cardStyle[slot.card.kind];
        const active = selected === slot.number;
        const warn = slot.hints.some((h) => h.severity === "warning");
        return (
          <button
            key={slot.number}
            onClick={() => onSelect?.(slot.number)}
            title={`${slot.location.label}: ${slot.card.name}`}
            className={`relative flex ${compact ? "h-12 w-14 text-xs" : "h-20 w-20 text-sm"} items-center justify-center rounded-xl font-bold shadow transition ${style.color} ${
              active ? "ring-4 ring-accent ring-offset-2 ring-offset-white dark:ring-offset-slate-900" : ""
            } ${slot.card.confidence !== "certain" && slot.card.kind !== "emptyOrPassive" ? "opacity-80" : ""}`}
          >
            {style.short}
            {slot.card.confidence === "likely" && slot.card.kind !== "emptyOrPassive" && (
              <span className="absolute right-1 top-0.5 text-[10px] font-normal">?</span>
            )}
            {warn && <span className="absolute -right-1.5 -top-1.5 h-3 w-3 rounded-full bg-red-500 ring-2 ring-white dark:ring-slate-900" />}
          </button>
        );
      })}
    </div>
  );

  return (
    <div className="flex items-stretch justify-center gap-2">
      {column(side("left"))}
      <div
        className={`flex ${compact ? "w-40" : "w-64"} flex-col rounded-2xl border-2 border-slate-300 bg-slate-50 p-3 dark:border-slate-700 dark:bg-slate-800/50`}
      >
        <div className="text-center text-[10px] uppercase tracking-widest text-slate-400">hinten</div>
        <div className="my-2 flex-1 rounded-lg bg-slate-200 dark:bg-slate-700/60" />
        <div className="mx-auto h-6 w-1/2 rounded-md bg-slate-200 dark:bg-slate-700/60" />
        <div className="mt-1 text-center text-[10px] uppercase tracking-widest text-slate-400">vorne</div>
      </div>
      {column(side("right"))}
    </div>
  );
}
