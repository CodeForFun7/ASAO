import React, { useState } from "react";
import { X, Sparkles, Key, Globe, Layers, Check, Info } from "lucide-react";
import { useAgentStore } from "../../stores/agent-store";

interface ConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ConfigModal: React.FC<ConfigModalProps> = ({ isOpen, onClose }) => {
  const config = useAgentStore((s) => s.config);
  const updateConfig = useAgentStore((s) => s.updateConfig);

  const [projectId, setProjectId] = useState(config.projectId || "");
  const [location, setLocation] = useState(config.location || "global");
  const [model, setModel] = useState(config.model || "gemini-3.5-flash-lite");
  const [apiKey, setApiKey] = useState(config.apiKey || "");
  const [savedNotice, setSavedNotice] = useState(false);

  if (!isOpen) return null;

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    updateConfig({
      projectId: projectId.trim(),
      location: location.trim() || "global",
      model: model.trim() || "gemini-3.5-flash-lite",
      apiKey: apiKey.trim(),
    });
    setSavedNotice(true);
    setTimeout(() => {
      setSavedNotice(false);
      onClose();
    }, 800);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
      <div className="w-full max-w-md rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        {/* Header */}
        <div className="px-5 py-4 border-b border-lunar-border flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="p-1.5 rounded-lg bg-lunar-surface-2 border border-lunar-border">
              <Sparkles className="w-4 h-4 text-lunar-ai" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-lunar-white tracking-tight">
                Vertex AI ADK Configuration
              </h2>
              <p className="text-[11px] text-lunar-text-sec">
                Google Cloud Vertex AI & Gemini Model Settings
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            className="p-1.5 rounded-md text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-surface-2 transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSave} className="p-5 space-y-4 text-xs">
          {/* Project ID */}
          <div className="space-y-1.5">
            <label className="flex items-center justify-between font-medium text-lunar-white">
              <span>Google Cloud Project ID</span>
              <span className="text-[10px] text-lunar-ai font-normal">Required</span>
            </label>
            <input
              type="text"
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
              placeholder="e.g. my-gcp-project-12345"
              className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec font-mono text-[11px]"
            />
            <p className="text-[10px] text-lunar-muted">
              Your Google Cloud project where the Vertex AI API is enabled.
            </p>
          </div>

          {/* Model Selection */}
          <div className="space-y-1.5">
            <label className="flex items-center justify-between font-medium text-lunar-white">
              <span className="flex items-center gap-1.5">
                <Layers className="w-3.5 h-3.5 text-lunar-text-sec" />
                <span>Model</span>
              </span>
              <span className="text-[10px] text-lunar-healthy font-medium">Recommended</span>
            </label>
            <input
              type="text"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="gemini-3.5-flash-lite"
              className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text font-mono text-[11px] focus:outline-none focus:border-lunar-text-sec"
            />
            <p className="text-[10px] text-lunar-muted">
              Configured for Vertex AI ADK with <strong className="text-lunar-white">gemini-3.5-flash-lite</strong>.
            </p>
          </div>

          {/* Location */}
          <div className="space-y-1.5">
            <label className="flex items-center gap-1.5 font-medium text-lunar-white">
              <Globe className="w-3.5 h-3.5 text-lunar-text-sec" />
              <span>Vertex AI Location</span>
            </label>
            <input
              type="text"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
              placeholder="global"
              className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text font-mono text-[11px] focus:outline-none focus:border-lunar-text-sec"
            />
            <p className="text-[10px] text-lunar-muted">
              Use <strong className="text-lunar-white">global</strong> for gemini-3.5-flash-lite model routing.
            </p>
          </div>

          {/* API Key or Token */}
          <div className="space-y-1.5">
            <label className="flex items-center justify-between font-medium text-lunar-white">
              <span className="flex items-center gap-1.5">
                <Key className="w-3.5 h-3.5 text-lunar-text-sec" />
                <span>API Key or Bearer Token</span>
              </span>
              <span className="text-[10px] text-lunar-muted font-normal">Optional</span>
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="AIzaSy... or OAuth Access Token"
              className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec font-mono text-[11px]"
            />
            <p className="text-[10px] text-lunar-muted">
              Enter your Google Cloud API key or OAuth token (e.g. from gcloud auth print-access-token).
            </p>
          </div>

          {/* Local Diagnostics Notice */}
          <div className="p-3 rounded-lg bg-lunar-bg border border-lunar-border/60 flex items-start gap-2.5 text-[11px] text-lunar-text-sec">
            <Info className="w-4 h-4 text-lunar-ai shrink-0 mt-0.5" />
            <div className="leading-relaxed">
              If no Project ID is configured, ASAO automatically uses its built-in native diagnostic engine so you can test all startup, process, and storage queries immediately.
            </div>
          </div>

          {/* Buttons */}
          <div className="pt-2 flex items-center justify-end gap-2.5">
            <button
              type="button"
              onClick={onClose}
              className="px-3.5 py-1.5 rounded-lg border border-lunar-border text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-surface-2 transition-colors cursor-pointer"
            >
              Cancel
            </button>

            <button
              type="submit"
              className="px-4 py-1.5 rounded-lg bg-lunar-white text-lunar-bg font-medium hover:bg-lunar-text transition-colors flex items-center gap-1.5 cursor-pointer shadow-sm"
            >
              {savedNotice && <Check className="w-3.5 h-3.5" />}
              <span>{savedNotice ? "Saved!" : "Save Configuration"}</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
