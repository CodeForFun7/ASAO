import React from "react";
import { ArrowUpDown, RotateCcw } from "lucide-react";
import type {
  StorageCategory,
  StorageImportance,
  StorageSortDirection,
  StorageSortField,
} from "../../types/storage";

interface FileFiltersProps {
  categoryFilter: StorageCategory | "ALL";
  onCategoryChange: (cat: StorageCategory | "ALL") => void;
  importanceFilter: StorageImportance | "ALL";
  onImportanceChange: (imp: StorageImportance | "ALL") => void;
  typeFilter: string | "ALL";
  onTypeChange: (ext: string | "ALL") => void;
  availableExtensions: string[];
  sortField: StorageSortField;
  sortDirection: StorageSortDirection;
  onSortChange: (field: StorageSortField, direction?: StorageSortDirection) => void;
  onResetAll: () => void;
}

const CATEGORY_OPTIONS: Array<{ value: StorageCategory | "ALL"; label: string }> = [
  { value: "ALL", label: "Category: All" },
  { value: "SYSTEM", label: "System" },
  { value: "APPLICATION", label: "Application" },
  { value: "USER", label: "User" },
  { value: "CACHE", label: "Cache" },
  { value: "TEMPORARY", label: "Temporary" },
  { value: "UNKNOWN", label: "Unknown" },
];

const IMPORTANCE_OPTIONS: Array<{ value: StorageImportance | "ALL"; label: string }> = [
  { value: "ALL", label: "Importance: All" },
  { value: "CRITICAL", label: "Critical" },
  { value: "IMPORTANT", label: "Important" },
  { value: "NORMAL", label: "Normal" },
  { value: "LOW", label: "Low" },
  { value: "UNKNOWN", label: "Unknown" },
];

const SORT_OPTIONS: Array<{ value: StorageSortField; label: string }> = [
  { value: "size", label: "Sort: Size" },
  { value: "name", label: "Sort: Name" },
  { value: "modified", label: "Sort: Last Modified" },
  { value: "accessed", label: "Sort: Last Accessed" },
  { value: "category", label: "Sort: Category" },
  { value: "importance", label: "Sort: Importance" },
];

export const FileFilters: React.FC<FileFiltersProps> = ({
  categoryFilter,
  onCategoryChange,
  importanceFilter,
  onImportanceChange,
  typeFilter,
  onTypeChange,
  availableExtensions,
  sortField,
  sortDirection,
  onSortChange,
  onResetAll,
}) => {
  const hasActiveFilters =
    categoryFilter !== "ALL" ||
    importanceFilter !== "ALL" ||
    typeFilter !== "ALL";

  return (
    <div className="flex flex-wrap items-center gap-2">
      {/* Category Select */}
      <select
        value={categoryFilter}
        onChange={(e) =>
          onCategoryChange(e.target.value as StorageCategory | "ALL")
        }
        className={`h-8 px-2.5 rounded border text-xs font-mono bg-lunar-surface focus:outline-none cursor-pointer transition-colors ${
          categoryFilter !== "ALL"
            ? "border-lunar-white/30 text-lunar-white bg-lunar-elevated"
            : "border-lunar-border text-lunar-text-sec hover:text-lunar-white"
        }`}
      >
        {CATEGORY_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value} className="bg-lunar-surface text-lunar-text">
            {opt.label}
          </option>
        ))}
      </select>

      {/* Importance Select */}
      <select
        value={importanceFilter}
        onChange={(e) =>
          onImportanceChange(e.target.value as StorageImportance | "ALL")
        }
        className={`h-8 px-2.5 rounded border text-xs font-mono bg-lunar-surface focus:outline-none cursor-pointer transition-colors ${
          importanceFilter !== "ALL"
            ? "border-lunar-white/30 text-lunar-white bg-lunar-elevated"
            : "border-lunar-border text-lunar-text-sec hover:text-lunar-white"
        }`}
      >
        {IMPORTANCE_OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value} className="bg-lunar-surface text-lunar-text">
            {opt.label}
          </option>
        ))}
      </select>

      {/* Dynamic File Type / Extension Select */}
      <select
        value={typeFilter}
        onChange={(e) => onTypeChange(e.target.value)}
        className={`h-8 px-2.5 rounded border text-xs font-mono bg-lunar-surface focus:outline-none cursor-pointer transition-colors ${
          typeFilter !== "ALL"
            ? "border-lunar-white/30 text-lunar-white bg-lunar-elevated"
            : "border-lunar-border text-lunar-text-sec hover:text-lunar-white"
        }`}
      >
        <option value="ALL" className="bg-lunar-surface text-lunar-text">
          Type: All
        </option>
        <option value="__FOLDER__" className="bg-lunar-surface text-lunar-text">
          Folders
        </option>
        {availableExtensions.map((ext) => (
          <option key={ext} value={ext} className="bg-lunar-surface text-lunar-text">
            .{ext.toUpperCase()}
          </option>
        ))}
      </select>

      {/* Sort Field + Direction */}
      <div className="flex items-center">
        <select
          value={sortField}
          onChange={(e) =>
            onSortChange(e.target.value as StorageSortField, sortDirection)
          }
          className="h-8 px-2.5 rounded-l border border-lunar-border text-xs font-mono bg-lunar-surface text-lunar-white focus:outline-none cursor-pointer"
        >
          {SORT_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value} className="bg-lunar-surface text-lunar-text">
              {opt.label}
            </option>
          ))}
        </select>

        <button
          type="button"
          onClick={() =>
            onSortChange(sortField, sortDirection === "desc" ? "asc" : "desc")
          }
          className="h-8 px-2 rounded-r border border-l-0 border-lunar-border bg-lunar-surface hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white text-[11px] font-mono flex items-center gap-1 cursor-pointer"
          title={`Direction: ${
            sortDirection === "desc" ? "Descending" : "Ascending"
          }`}
        >
          <ArrowUpDown className="w-3 h-3" />
          <span>{sortDirection.toUpperCase()}</span>
        </button>
      </div>

      {hasActiveFilters && (
        <button
          type="button"
          onClick={onResetAll}
          className="h-8 px-2.5 rounded border border-lunar-border bg-lunar-surface hover:bg-lunar-elevated text-xs font-mono text-lunar-text-sec hover:text-lunar-white flex items-center gap-1.5 cursor-pointer"
          title="Reset active filters"
        >
          <RotateCcw className="w-3 h-3" />
          <span>Reset</span>
        </button>
      )}
    </div>
  );
};
