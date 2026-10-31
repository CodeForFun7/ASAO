import { invoke } from "@tauri-apps/api/core";
import type { AuditPage, RestoreResult } from "../types/audit";

export const AUDIT_PAGE_SIZE = 100;

/** Changes made through ASAO, newest first. */
export async function listAuditEntries(limit = AUDIT_PAGE_SIZE, offset = 0): Promise<AuditPage> {
  return invoke<AuditPage>("audit_list", { limit, offset });
}

export async function restoreAuditEntry(id: number): Promise<RestoreResult> {
  return invoke<RestoreResult>("audit_restore", { id });
}
