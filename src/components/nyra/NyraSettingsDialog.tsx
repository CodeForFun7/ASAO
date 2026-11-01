import React, { useEffect, useState } from "react";
import { DEFAULT_MODEL, deleteOnlineChats, getCloudStatus, syncChatsNow } from "../../services/nyra";
import type { CloudStatus, NyraSettings } from "../../types/nyra";

interface Props {
  settings: NyraSettings;
  onSave: (settings: NyraSettings) => void;
  onClose: () => void;
}

function syncedAgo(ms: number): string {
  const minutes = Math.round((Date.now() - ms) / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  return new Date(ms).toLocaleString();
}

export const NyraSettingsDialog: React.FC<Props> = ({ settings, onSave, onClose }) => {
  const [draft, setDraft] = useState<NyraSettings>(settings);
  const [cloud, setCloud] = useState<CloudStatus | null>(null);
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudNote, setCloudNote] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    getCloudStatus().then(setCloud, () => setCloud(null));
  }, []);

  const runCloud = async (action: () => Promise<unknown>, done: string) => {
    setCloudBusy(true);
    setCloudNote("");
    try {
      await action();
      setCloudNote(done);
    } catch (err) {
      setCloudNote(err instanceof Error ? err.message : String(err));
    } finally {
      setCloud(await getCloudStatus().catch(() => null));
      setCloudBusy(false);
    }
  };
  // The switch is saved with the dialog; the buttons act on what Nyra has now.
  const syncOnNow = cloud?.enabled ?? settings.syncChats;
  const field = "mt-1 w-full rounded-lg border border-lunar-border bg-lunar-bg px-3 py-2 text-sm outline-none focus:border-lunar-muted";
  return (
    <div className="absolute inset-0 z-20 bg-black/50 flex items-center justify-center p-5" onClick={onClose}>
      <div
        className="w-full max-w-md rounded-2xl border border-lunar-border bg-lunar-surface p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-sm font-semibold text-lunar-white">Nyra settings</h2>
        <p className="text-xs text-lunar-text-sec mt-1">
          Nyra uses an OpenAI-compatible model. The key is stored in this app on this device and sent only to the local
          Nyra service, which passes it to the model provider.
        </p>
        <label className="block mt-4 text-[11px] text-lunar-text-sec">
          API key
          <input
            type="password"
            value={draft.apiKey}
            onChange={(e) => setDraft({ ...draft, apiKey: e.target.value })}
            placeholder="sk-…"
            className={field}
          />
        </label>
        <label className="block mt-3 text-[11px] text-lunar-text-sec">
          Model
          <input
            value={draft.model}
            onChange={(e) => setDraft({ ...draft, model: e.target.value })}
            placeholder={DEFAULT_MODEL}
            className={field}
          />
        </label>
        <label className="block mt-3 text-[11px] text-lunar-text-sec">
          Endpoint (optional)
          <input
            value={draft.baseUrl}
            onChange={(e) => setDraft({ ...draft, baseUrl: e.target.value })}
            placeholder="https://api.openai.com/v1"
            className={field}
          />
          <span className="block mt-1 text-[10px] text-lunar-muted">For OpenAI-compatible providers. Leave empty for OpenAI.</span>
        </label>
        <div className="mt-4 pt-4 border-t border-lunar-border/60">
          <label className="flex items-start gap-2.5 cursor-pointer">
            <input
              type="checkbox"
              checked={draft.syncChats}
              onChange={(e) => setDraft({ ...draft, syncChats: e.target.checked })}
              className="mt-0.5 accent-current"
            />
            <span>
              <span className="block text-xs text-lunar-text">Save chats to my account</span>
              <span className="block text-[10px] text-lunar-muted mt-0.5 leading-relaxed">
                Your chats are stored online with your ASAO account and appear when you sign in on another PC. They
                can include details about this PC, such as app names, file paths and running processes. The API key is
                never uploaded.
              </span>
            </span>
          </label>
          {cloud && cloud.signedIn && (
            <div className="mt-2 pl-6 text-[10px] text-lunar-muted space-y-1.5">
              <p>
                {cloud.enabled
                  ? cloud.syncing
                    ? "Syncing…"
                    : cloud.error
                      ? `Last sync failed: ${cloud.error}`
                      : cloud.lastSyncAt
                        ? `Synced ${syncedAgo(cloud.lastSyncAt)}.`
                        : "Waiting to sync."
                  : "Syncing is off. New chats stay on this PC."}
              </p>
              <div className="flex gap-3">
                {syncOnNow ? (
                  <button
                    type="button"
                    disabled={cloudBusy}
                    onClick={() => void runCloud(syncChatsNow, "Synced.")}
                    className="text-lunar-text-sec hover:text-lunar-white disabled:opacity-50"
                  >
                    Sync now
                  </button>
                ) : (
                  <button
                    type="button"
                    disabled={cloudBusy}
                    onClick={() => {
                      if (!confirmDelete) {
                        setConfirmDelete(true);
                        return;
                      }
                      setConfirmDelete(false);
                      void runCloud(deleteOnlineChats, "Chats saved online were deleted. Chats on this PC are kept.");
                    }}
                    className={`disabled:opacity-50 ${
                      confirmDelete ? "text-lunar-critical font-medium" : "text-lunar-text-sec hover:text-lunar-critical"
                    }`}
                  >
                    {confirmDelete ? "Click again to delete them" : "Delete chats saved online"}
                  </button>
                )}
              </div>
              {cloudNote && <p className="text-lunar-text-sec">{cloudNote}</p>}
              {!syncOnNow && (
                <p>Other PCs where syncing is still on upload their chats again.</p>
              )}
            </div>
          )}
        </div>
        <div className="flex justify-between mt-5">
          <button
            type="button"
            onClick={() => setDraft({ ...draft, apiKey: "" })}
            className="text-xs text-lunar-text-sec hover:text-lunar-critical"
          >
            Remove key
          </button>
          <div className="flex gap-2">
            <button type="button" onClick={onClose} className="rounded-lg px-3 py-2 text-xs text-lunar-text-sec hover:bg-lunar-surface-2">
              Cancel
            </button>
            <button
              type="button"
              onClick={() => {
                onSave({ ...draft, model: draft.model.trim() || DEFAULT_MODEL });
                onClose();
              }}
              className="rounded-lg bg-lunar-white px-3 py-2 text-xs text-lunar-bg"
            >
              Save
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
