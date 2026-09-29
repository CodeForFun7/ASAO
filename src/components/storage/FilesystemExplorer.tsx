import React, { useMemo } from "react";
import {
  STORAGE_IMPORTANCE_META,
  type StorageCategory,
  type StorageDirectoryListing,
  type StorageImportance,
  type StorageItem,
  type StorageSortDirection,
  type StorageSortField,
} from "../../types/storage";
import { Breadcrumbs } from "./Breadcrumbs";
import { FileToolbar } from "./FileToolbar";
import { FileTable } from "./FileTable";

interface FilesystemExplorerProps {
  currentPath: string;
  directoryListing: StorageDirectoryListing | null;
  isLoadingDirectory: boolean;
  searchQuery: string;
  searchResults: StorageItem[] | null;
  isSearching: boolean;
  categoryFilter: StorageCategory | "ALL";
  importanceFilter: StorageImportance | "ALL";
  typeFilter: string | "ALL";
  discoveredExtensions: string[];
  sortField: StorageSortField;
  sortDirection: StorageSortDirection;
  selectedItem: StorageItem | null;
  onNavigate: (path: string) => void;
  onRetryDirectory: () => void;
  onSearchChange: (q: string) => void;
  onCategoryChange: (cat: StorageCategory | "ALL") => void;
  onImportanceChange: (imp: StorageImportance | "ALL") => void;
  onTypeChange: (ext: string | "ALL") => void;
  onSortChange: (field: StorageSortField, dir?: StorageSortDirection) => void;
  onSelectItem: (item: StorageItem) => void;
  onResetFilters: () => void;
}

export const FilesystemExplorer: React.FC<FilesystemExplorerProps> = ({
  currentPath,
  directoryListing,
  isLoadingDirectory,
  searchQuery,
  searchResults,
  isSearching,
  categoryFilter,
  importanceFilter,
  typeFilter,
  discoveredExtensions,
  sortField,
  sortDirection,
  selectedItem,
  onNavigate,
  onRetryDirectory,
  onSearchChange,
  onCategoryChange,
  onImportanceChange,
  onTypeChange,
  onSortChange,
  onSelectItem,
  onResetFilters,
}) => {
  // Base items: either search results (if searching) or current directory listing
  const baseItems: StorageItem[] = useMemo(() => {
    if (searchQuery.trim().length > 0 && searchResults !== null) {
      return searchResults;
    }
    return directoryListing?.items ?? [];
  }, [searchQuery, searchResults, directoryListing]);

  // Combine extensions from current directory + drive-wide discovered extensions
  const availableExtensions = useMemo(() => {
    const extSet = new Set<string>(discoveredExtensions);
    for (const item of baseItems) {
      if (!item.isDir && item.extension) {
        extSet.add(item.extension.toLowerCase());
      }
    }
    return Array.from(extSet).sort();
  }, [discoveredExtensions, baseItems]);

  // Apply Category, Importance, Type filters and Sorting
  const filteredAndSortedItems = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();

    const filtered = baseItems.filter((item) => {
      if (categoryFilter !== "ALL" && item.category !== categoryFilter) {
        return false;
      }
      if (importanceFilter !== "ALL" && item.importance !== importanceFilter) {
        return false;
      }
      if (typeFilter !== "ALL") {
        if (typeFilter === "__FOLDER__") {
          if (!item.isDir) return false;
        } else if (item.isDir || item.extension.toLowerCase() !== typeFilter.toLowerCase()) {
          return false;
        }
      }
      if (q.length > 0 && searchResults === null) {
        const matchName = item.name.toLowerCase().includes(q);
        const matchPath = item.path.toLowerCase().includes(q);
        const matchExt = item.extension.toLowerCase().includes(q.replace(/^\./, ""));
        if (!matchName && !matchPath && !matchExt) {
          return false;
        }
      }
      return true;
    });

    const dirMultiplier = sortDirection === "desc" ? -1 : 1;

    return [...filtered].sort((a, b) => {
      switch (sortField) {
        case "name":
          return dirMultiplier * a.name.localeCompare(b.name);
        case "modified":
          return (
            dirMultiplier * ((a.modifiedMs ?? 0) - (b.modifiedMs ?? 0)) ||
            b.size - a.size
          );
        case "accessed":
          return (
            dirMultiplier * ((a.accessedMs ?? 0) - (b.accessedMs ?? 0)) ||
            b.size - a.size
          );
        case "category":
          return (
            dirMultiplier * a.category.localeCompare(b.category) ||
            b.size - a.size
          );
        case "importance": {
          const pA = STORAGE_IMPORTANCE_META[a.importance]?.priority ?? 99;
          const pB = STORAGE_IMPORTANCE_META[b.importance]?.priority ?? 99;
          return dirMultiplier * (pA - pB) || b.size - a.size;
        }
        case "size":
        default:
          return dirMultiplier * (a.size - b.size) || a.name.localeCompare(b.name);
      }
    });
  }, [
    baseItems,
    categoryFilter,
    importanceFilter,
    typeFilter,
    searchQuery,
    searchResults,
    sortField,
    sortDirection,
  ]);

  return (
    <section className="space-y-3">
      {/* Header & Breadcrumbs */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
        <div className="space-y-1.5">
          <div className="flex items-center gap-3">
            <h2 className="text-sm font-semibold text-lunar-white tracking-tight">
              Files
            </h2>
            <span className="text-xs font-mono text-lunar-muted">
              {filteredAndSortedItems.length}{" "}
              {filteredAndSortedItems.length === 1 ? "item" : "items"}
              {searchQuery.trim() ? ` matching "${searchQuery.trim()}"` : ""}
            </span>
          </div>
          <Breadcrumbs
            currentPath={currentPath}
            parentPath={directoryListing?.parentPath ?? null}
            onNavigate={onNavigate}
          />
        </div>
      </div>

      {/* Main Explorer Card: Toolbar + Table */}
      <div className="rounded-lg lunar-glass-card overflow-hidden">
        <FileToolbar
          searchQuery={searchQuery}
          onSearchChange={onSearchChange}
          isSearching={isSearching}
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
          onResetAll={onResetFilters}
        />

        <FileTable
          items={filteredAndSortedItems}
          totalUnfilteredCount={baseItems.length}
          isLoading={isLoadingDirectory}
          permissionDenied={directoryListing?.permissionDenied ?? false}
          selectedPath={selectedItem?.path ?? null}
          sortField={sortField}
          sortDirection={sortDirection}
          onSortColumn={(field) => onSortChange(field)}
          onSelectItem={onSelectItem}
          onOpenFolder={onNavigate}
          onRetryPermission={onRetryDirectory}
          onResetFilters={onResetFilters}
        />
      </div>
    </section>
  );
};
