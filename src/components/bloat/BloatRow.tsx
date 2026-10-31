import React from "react";
import { ChevronRight } from "lucide-react";
import type { BloatItem } from "../../types/bloat";
import { KIND_LABELS } from "../../types/bloat";
import { formatStorageBytes } from "../../services/storage";
import { LeftoverCheckbox } from "../storage/leftovers/LeftoverPrimitives";
import { CategoryChip, ImpactScore, SafetyBadge } from "./BloatPrimitives";

interface BloatRowProps {
  item: BloatItem;
  selected: boolean;
  onToggle: () => void;
  onOpen: () => void;
}

export const BloatRow: React.FC<BloatRowProps> = ({ item, selected, onToggle, onOpen }) => {
  const extras = [
    item.services.length > 0 && `${item.services.length} ${item.services.length === 1 ? "service" : "services"}`,
    item.startup.length > 0 && `${item.startup.length} startup`,
    item.tasks.length > 0 && `${item.tasks.length} ${item.tasks.length === 1 ? "task" : "tasks"}`,
  ].filter(Boolean);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter") onOpen();
      }}
      className="flex items-center gap-3 px-4 py-3 rounded-lg lunar-glass-card hover:border-lunar-text-sec/40 transition-colors cursor-pointer"
    >
      <div className="w-4 shrink-0">
        {item.selectable && (
          <LeftoverCheckbox
            state={selected ? "checked" : "unchecked"}
            label={`Select ${item.name}`}
            onToggle={onToggle}
          />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xs font-semibold text-lunar-white truncate" title={item.name}>
            {item.name}
          </span>
          {item.running && (
            <span className="w-1.5 h-1.5 rounded-full bg-lunar-healthy shrink-0" title="Running now" />
          )}
        </div>
        <div className="flex items-center gap-1.5 mt-1 min-w-0 flex-wrap">
          <span className="text-[11px] text-lunar-text-sec truncate max-w-[220px]">
            {item.publisher ?? KIND_LABELS[item.kind]}
          </span>
          {item.category && <CategoryChip category={item.category} />}
          {extras.length > 0 && <span className="text-[10px] text-lunar-muted font-mono">{extras.join(" · ")}</span>}
        </div>
      </div>

      <div className="hidden sm:block w-20 text-right shrink-0">
        <div className="text-[10px] text-lunar-muted">Size</div>
        <div className="text-xs font-mono text-lunar-white">
          {item.sizeBytes ? formatStorageBytes(item.sizeBytes) : "—"}
        </div>
      </div>
      <div className="w-24 shrink-0">
        <ImpactScore total={item.impact.total} band={item.impact.band} />
      </div>
      <div className="w-28 flex justify-end shrink-0">
        <SafetyBadge safety={item.safety} />
      </div>
      <ChevronRight className="w-4 h-4 text-lunar-muted shrink-0" />
    </div>
  );
};
