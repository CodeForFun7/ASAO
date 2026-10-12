import React, { useMemo } from "react";
import type { StartupGroup, StartupItem } from "../../types/startup";
import { STARTUP_GROUP_DESCRIPTIONS } from "../../types/startup";

interface StartupCategoryTabsProps {
  items: StartupItem[];
  activeGroup: StartupGroup | "all";
  onSelectGroup: (group: StartupGroup | "all") => void;
}

interface CategoryMeta {
  key: StartupGroup | "all";
  label: string;
  shortLabel: string;
  description: string;
}

const CATEGORIES: CategoryMeta[] = [
  {
    key: "all",
    label: "All Startup Items",
    shortLabel: "All",
    description: "Every detected autostart entry across all Windows mechanisms",
  },
  {
    key: "Registry Startup",
    label: "Registry Startup",
    shortLabel: "Registry",
    description: STARTUP_GROUP_DESCRIPTIONS["Registry Startup"],
  },
  {
    key: "Startup Folder",
    label: "Startup Folder",
    shortLabel: "Folders",
    description: STARTUP_GROUP_DESCRIPTIONS["Startup Folder"],
  },
  {
    key: "Scheduled Tasks",
    label: "Scheduled Tasks",
    shortLabel: "Tasks",
    description: STARTUP_GROUP_DESCRIPTIONS["Scheduled Tasks"],
  },
  {
    key: "Windows Services",
    label: "Windows Services",
    shortLabel: "Services",
    description: STARTUP_GROUP_DESCRIPTIONS["Windows Services"],
  },
  {
    key: "Winlogon / System Startup",
    label: "Winlogon / System",
    shortLabel: "Winlogon",
    description: STARTUP_GROUP_DESCRIPTIONS["Winlogon / System Startup"],
  },
  {
    key: "Other Autostart Mechanisms",
    label: "Other Mechanisms",
    shortLabel: "Other",
    description: STARTUP_GROUP_DESCRIPTIONS["Other Autostart Mechanisms"],
  },
];

export const StartupCategoryTabs: React.FC<StartupCategoryTabsProps> = ({
  items,
  activeGroup,
  onSelectGroup,
}) => {
  const counts = useMemo(() => {
    const map: Record<StartupGroup | "all", number> = {
      all: items.length,
      "Registry Startup": 0,
      "Startup Folder": 0,
      "Scheduled Tasks": 0,
      "Windows Services": 0,
      "Winlogon / System Startup": 0,
      "Other Autostart Mechanisms": 0,
    };

    for (const item of items) {
      if (map[item.startupGroup] !== undefined) {
        map[item.startupGroup] += 1;
      } else {
        map["Other Autostart Mechanisms"] += 1;
      }
    }

    return map;
  }, [items]);

  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-2">
      {CATEGORIES.map((cat) => {
        const isSelected = activeGroup === cat.key;
        const count = counts[cat.key] ?? 0;

        return (
          <button
            key={cat.key}
            type="button"
            onClick={() => onSelectGroup(cat.key)}
            title={cat.description}
            className={`text-left px-3 py-2 rounded-xl transition-all cursor-pointer flex flex-col justify-between min-h-[54px] ${
              isSelected
                ? "lunar-glass-card border-lunar-white/30 text-lunar-white"
                : "lunar-glass-card-interactive text-lunar-text-sec hover:text-lunar-text"
            }`}
          >
            <span className="text-[11px] font-medium truncate w-full">
              {cat.shortLabel}
            </span>
            <span
              className={`font-mono text-sm font-semibold ${
                isSelected ? "text-lunar-white" : "text-lunar-text"
              }`}
            >
              {count}
            </span>
          </button>
        );
      })}
    </div>
  );
};
