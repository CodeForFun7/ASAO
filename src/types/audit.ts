export interface AuditEntry {
  id: number;
  createdAt: number;
  category: string;
  title: string;
  detail: string;
  /** The Restore button works: the change can be reversed and has not been yet. */
  restorable: boolean;
  /** Why the change cannot be restored. */
  undoNote: string | null;
  restoredAt: number | null;
}

export interface AuditPage {
  entries: AuditEntry[];
  total: number;
}

export interface RestoreResult {
  ok: boolean;
  message: string;
}

export const AUDIT_CATEGORY_LABELS: Record<string, string> = {
  startup: "Startup",
  debloat: "Debloat",
  leftovers: "Leftovers",
  apps: "Unused apps",
  files: "Files",
  "recycle-bin": "Recycle Bin",
  processes: "Processes",
  "ignore-list": "Ignore list",
  restore: "Restore",
};
