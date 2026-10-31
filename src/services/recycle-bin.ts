import { invoke } from "@tauri-apps/api/core";
import type { RecycleBinDeleteReport, RecycleBinListing } from "../types/recycle-bin";

export async function listRecycleBin(): Promise<RecycleBinListing> {
  return invoke<RecycleBinListing>("recycle_bin_list");
}

/** Permanently deletes the chosen items, by their path inside $Recycle.Bin. */
export async function deleteRecycleBinItems(paths: string[]): Promise<RecycleBinDeleteReport> {
  return invoke<RecycleBinDeleteReport>("recycle_bin_delete", { paths });
}

export async function openRecycleBin(): Promise<void> {
  return invoke<void>("residual_open_recycle_bin");
}
