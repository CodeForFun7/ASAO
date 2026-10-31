export interface RecycleBinItem {
  name: string;
  /** Where the item sits inside $Recycle.Bin. */
  path: string;
  /** The folder it was deleted from. */
  originalFolder: string | null;
  isFolder: boolean;
  typeName: string | null;
  sizeBytes: number | null;
  deletedMs: number | null;
}

export interface RecycleBinListing {
  items: RecycleBinItem[];
  /** Totals from Windows, across every drive. */
  totalBytes: number;
  totalItems: number;
}

export interface RecycleBinDeleteFailure {
  path: string;
  reason: string;
}

export interface RecycleBinDeleteReport {
  deleted: number;
  freedBytes: number;
  failed: RecycleBinDeleteFailure[];
  /** What is in the bin afterwards. */
  listing: RecycleBinListing;
}

export type RecycleBinSortField = "deleted" | "size" | "name";
