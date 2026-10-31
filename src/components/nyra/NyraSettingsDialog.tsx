import React, { useState } from "react";
import { DEFAULT_MODEL } from "../../services/nyra";
import type { NyraSettings } from "../../types/nyra";

interface Props {
  settings: NyraSettings;
  onSave: (settings: NyraSettings) => void;
  onClose: () => void;
}

export const NyraSettingsDialog: React.FC<Props> = ({ settings, onSave, onClose }) => {
  const [draft, setDraft] = useState<NyraSettings>(settings);
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
