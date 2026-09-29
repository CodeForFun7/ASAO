import React from "react";
import { Search, X } from "lucide-react";
import type {
  StorageCategory,
  StorageImportance,
  StorageSortDirection,
  StorageSortField,
} from "../../types/storage";
import { FileFilters } from "./FileFilters";

interface FileToolbarProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  isSearching: boolean;
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

export const FileToolbar: React.FC<FileToolbarProps> = ({
  searchQuery,
  onSearchChange,
  isSearching,
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
  return (
    <div className="p-3 bg-lunar-surface-2/60 border-b border-lunar-border flex flex-wrap items-center justify-between gap-2.5">
      {/* Search Input */}
      <div className="relative flex-1 min-w-[220px] max-w-sm">
        <Search className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Search files, folders, extensions, paths..."
          className="w-full h-8 pl-8 pr-7 bg-lunar-surface border border-lunar-border rounded text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-white/40 transition-colors"
        />
        {isSearching && (
          <div className="w-3 h-3 rounded-full border border-lunar-border border-t-lunar-white animate-spin absolute right-7 top-1/2 -translate-y-1/2" />
        )}
        {searchQuery && (
          <button
            type="button"
            onClick={() => onSearchChange("")}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-lunar-muted hover:text-lunar-white cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Filters & Sorting */}
      <FileFilters
        categoryFilter={categoryFilter}
        onCategoryChange={onCategoryChange}
        importanceFilter={importanceFilter}
        onImportanceChange={onImportanceChange}
        typeFilter={typeFilter}
        onTypeChange={onTypeChange}
        availableExtensions={availableExtensions}
        sortField={sortField}
        sortDirection={sortDirection}
        onSortChange={onSortChange}
        onResetAll={onResetAll}
      />
    </div>
  );
};
