import type { Slot } from "../bindings/Slot";
import { cardStyle } from "../format";
import { isSilent, slotRows } from "../slots";

const KEY_ROWS = [14, 14, 13, 12, 8];

/** Top view of the laptop: hinge at the back, keyboard and touchpad, cards plugged into the sides. */
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
  const card = (slot: Slot | undefined, side: "left" | "right") => {
    if (!slot) return <div />;
    const style = cardStyle[slot.card.kind];
    const active = selected === slot.number;
    const warn = slot.hints.some((h) => h.severity === "warning");
    const silent = isSilent(slot);
    const unsure = slot.card.confidence !== "certain" && slot.card.kind !== "emptyOrPassive";
    return (
      <button
        key={slot.number}
        onClick={() => onSelect?.(slot.number)}
        title={`${slot.location.label}: ${slot.card.name}${silent ? " (gemerkt, meldet sich gerade nicht)" : ""}`}
        className={`relative flex ${compact ? "h-11 w-16 text-xs" : "h-16 w-24 text-sm"} items-center justify-center font-bold shadow-md transition hover:brightness-110 focus-visible:outline-2 focus-visible:outline-accent ${
          side === "left" ? "rounded-l-xl rounded-r-md" : "rounded-r-xl rounded-l-md"
        } ${style.color} ${
          active ? "ring-4 ring-accent ring-offset-2 ring-offset-slate-100 dark:ring-offset-slate-900" : ""
        } ${silent ? "opacity-60 outline-2 -outline-offset-4 outline-dashed outline-white/80" : unsure ? "opacity-80" : ""}`}
      >
        {/* port opening on the outer edge */}
        <span
          className={`absolute top-1/2 h-1/2 w-1 -translate-y-1/2 rounded-full bg-black/25 ${
            side === "left" ? "left-1" : "right-1"
          }`}
        />
        {style.short}
        {slot.card.confidence === "likely" && slot.card.kind !== "emptyOrPassive" && (
          <span className="absolute right-1.5 top-0.5 text-[10px] font-normal">?</span>
        )}
        {silent && (
          <span className="absolute right-1.5 top-0.5 text-[10px] font-normal" aria-label="gemerkt">
            ↺
          </span>
        )}
        {warn && (
          <span className="absolute -top-1.5 left-1/2 h-3 w-3 -translate-x-1/2 rounded-full bg-red-500 ring-2 ring-white dark:ring-slate-900" />
        )}
      </button>
    );
  };

  const overhang = compact ? "2rem" : "3rem";

  return (
    <div className="flex justify-center" style={{ paddingInline: overhang }}>
      <div
        className={`relative ${compact ? "h-52 w-72" : "h-80 w-[26rem]"} max-w-full rounded-2xl border-2 border-slate-300 bg-slate-100 shadow-inner dark:border-slate-600 dark:bg-slate-800`}
      >
        <div className="mx-auto mt-1 h-1.5 w-2/3 rounded-full bg-slate-300 dark:bg-slate-600" title="Scharnier" />
        <div className="text-center text-[10px] uppercase tracking-widest text-slate-400">hinten</div>

        {/* keyboard */}
        <div className="mx-auto mt-2 flex w-[62%] flex-col gap-[3px]">
          {KEY_ROWS.map((n, i) => (
            <div key={i} className="flex justify-center gap-[3px]">
              {Array.from({ length: n }, (_, k) => (
                <span key={k} className={`${compact ? "h-2" : "h-3.5"} flex-1 rounded-[2px] bg-slate-200 dark:bg-slate-700`} />
              ))}
            </div>
          ))}
        </div>

        {/* touchpad */}
        <div
          className={`absolute bottom-6 left-1/2 ${compact ? "h-10 w-24" : "h-16 w-36"} -translate-x-1/2 rounded-lg border border-slate-300 bg-slate-200/70 dark:border-slate-600 dark:bg-slate-700/60`}
        />
        <div className="absolute inset-x-0 bottom-1 text-center text-[10px] uppercase tracking-widest text-slate-400">vorne</div>

        {/* cards sticking out of both sides */}
        <div
          className="absolute inset-y-6 flex flex-col justify-around"
          style={{ left: `calc(-${overhang} + 0.75rem)`, right: `calc(-${overhang} + 0.75rem)` }}
        >
          {slotRows(slots).map((row) => (
            <div key={row.position} className="flex items-center justify-between">
              {card(row.left, "left")}
              {card(row.right, "right")}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
