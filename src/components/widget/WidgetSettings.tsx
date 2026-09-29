import React, { useEffect, useState } from "react";
import {
  AppWindow,
  Eye,
  EyeOff,
  Sliders,
  Sparkles,
  Globe,
  Layers,
  Key,
  ArrowUpRight,
  Check,
} from "lucide-react";
import { useWidgetStore } from "../../stores/widget-store";
import { useAgentStore } from "../../stores/agent-store";
import { useProcessStore } from "../../stores/process-store";
import { hideWidget } from "../../services/widget";

const OPACITY_PRESETS = [60, 70, 75, 80, 90, 100];

interface ToggleRowProps {
  title: string;
  description: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

const ToggleRow: React.FC<ToggleRowProps> = ({
  title,
  description,
  checked,
  disabled = false,
  onChange,
}) => (
  <div
    className={`flex items-center justify-between py-3.5 border-b border-lunar-border/60 last:border-b-0 ${
      disabled ? "opacity-45 pointer-events-none" : ""
    }`}
  >
    <div className="pr-4">
      <div className="text-xs font-medium text-lunar-white">{title}</div>
      <div className="text-[11px] text-lunar-text-sec mt-0.5">
        {description}
      </div>
    </div>

    <button
      type="button"
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`w-10 h-5 rounded-full border transition-colors relative shrink-0 cursor-pointer ${
        checked
          ? "bg-lunar-white border-lunar-white"
          : "bg-lunar-bg border-lunar-border"
      }`}
    >
      <span
        className={`block w-3.5 h-3.5 rounded-full transition-transform ${
          checked
            ? "translate-x-5 bg-lunar-bg"
            : "translate-x-0.5 bg-lunar-text-sec"
        }`}
      />
    </button>
  </div>
);

export const WidgetSettings: React.FC = () => {
  const settings = useWidgetStore((s) => s.settings);
  const widgetVisible = useWidgetStore((s) => s.widgetVisible);
  const initializeWidget = useWidgetStore((s) => s.initializeWidget);
  const toggleWidgetVisibility = useWidgetStore(
    (s) => s.toggleWidgetVisibility
  );
  const updateSettingsPatch = useWidgetStore((s) => s.updateSettingsPatch);

  const agentConfig = useAgentStore((s) => s.config);
  const updateAgentConfig = useAgentStore((s) => s.updateConfig);
  const setRoute = useProcessStore((s) => s.setRoute);

  const [aiProjectId, setAiProjectId] = useState(agentConfig.projectId || "");
  const [aiApiKey, setAiApiKey] = useState(agentConfig.apiKey || "");
  const [aiSaved, setAiSaved] = useState(false);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;
    void initializeWidget().then((unlisten) => {
      if (mounted) cleanup = unlisten;
      else unlisten();
    });
    return () => {
      mounted = false;
      if (cleanup) cleanup();
    };
  }, [initializeWidget]);

  const handleToggleWidgetEnabled = async (enabled: boolean) => {
    await updateSettingsPatch({ widgetEnabled: enabled });
    if (!enabled) {
      await hideWidget();
    }
  };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      {/* Page Header (Show/Hide top buttons removed) */}
      <div>
        <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
          Settings
        </h1>
        <p className="text-xs text-lunar-text-sec mt-0.5">
          Configure ASAO system startup, desktop companion widget, and telemetry preferences.
        </p>
      </div>

      {/* Main Settings Content */}
      <div className="w-full max-w-3xl mx-auto space-y-6">
          {/* Startup & Background Service Section */}
          <section className="rounded-xl lunar-glass-card p-5">
            <div className="flex items-center gap-2 mb-3">
              <Sliders className="w-3.5 h-3.5 text-lunar-text-sec" />
              <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
                System &amp; Startup Behavior
              </h2>
            </div>

            <div className="divide-y divide-lunar-border/60">
              <ToggleRow
                title="Start Asao with Windows"
                description="Launch Asao's background telemetry analyzer automatically when signing into Windows."
                checked={settings.startWithWindows}
                onChange={(val) =>
                  void updateSettingsPatch({ startWithWindows: val })
                }
              />
            </div>
          </section>

          {/* Desktop Widget Configuration Section */}
          <section className="rounded-xl lunar-glass-card p-5">
            <div className="flex items-center gap-2 mb-3">
              <AppWindow className="w-3.5 h-3.5 text-lunar-white" />
              <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
                Desktop Widget
              </h2>
            </div>

            <div>
              <ToggleRow
                title="Desktop Widget"
                description="Enable the companion desktop widget and the 'Show Widget' system tray control."
                checked={settings.widgetEnabled}
                onChange={(val) => void handleToggleWidgetEnabled(val)}
              />

              {/* When Desktop Widget is enabled: Show/Hide single toggle button + Opacity selection right below it */}
              {settings.widgetEnabled && (
                <div className="py-4 border-b border-lunar-border/60 space-y-4">
                  {/* Single Show/Hide Toggle Button */}
                  <div className="flex items-center justify-between">
                    <div className="pr-4">
                      <div className="text-xs font-medium text-lunar-white">
                        Widget Visibility
                      </div>
                      <div className="text-[11px] text-lunar-text-sec mt-0.5">
                        Show or hide the desktop widget on your screen. Drag it anywhere to reposition.
                      </div>
                    </div>

                    <button
                      type="button"
                      onClick={() => void toggleWidgetVisibility()}
                      className={`h-8 px-3.5 rounded-md border text-xs font-medium flex items-center gap-1.5 transition-colors shrink-0 cursor-pointer ${
                        widgetVisible
                          ? "bg-lunar-white text-lunar-bg border-lunar-white hover:bg-lunar-text"
                          : "bg-lunar-elevated text-lunar-white border-lunar-border hover:bg-lunar-surface-2"
                      }`}
                    >
                      {widgetVisible ? (
                        <>
                          <EyeOff className="w-3.5 h-3.5" />
                          <span>Hide Widget</span>
                        </>
                      ) : (
                        <>
                          <Eye className="w-3.5 h-3.5" />
                          <span>Show Widget</span>
                        </>
                      )}
                    </button>
                  </div>

                  {/* Opacity Selection Directly Below */}
                  <div className="pt-2 border-t border-lunar-border/40 space-y-2.5">
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-lunar-white font-medium">
                        Widget Opacity
                      </span>
                      <span className="text-lunar-white font-semibold">
                        {settings.widgetOpacity}%
                      </span>
                    </div>

                    <input
                      type="range"
                      min={60}
                      max={100}
                      step={5}
                      value={settings.widgetOpacity}
                      onChange={(e) =>
                        void updateSettingsPatch({
                          widgetOpacity: Number(e.target.value),
                        })
                      }
                      className="w-full accent-lunar-white cursor-pointer"
                    />

                    <div className="flex items-center justify-between gap-1.5">
                      {OPACITY_PRESETS.map((preset) => (
                        <button
                          key={preset}
                          type="button"
                          onClick={() =>
                            void updateSettingsPatch({ widgetOpacity: preset })
                          }
                          className={`flex-1 py-1 rounded border text-[11px] transition-colors cursor-pointer ${
                            settings.widgetOpacity === preset
                              ? "bg-lunar-elevated border-lunar-text-sec text-lunar-white"
                              : "bg-lunar-bg border-lunar-border text-lunar-muted hover:text-lunar-text-sec"
                          }`}
                        >
                          {preset}%
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              )}

              <ToggleRow
                title="Launch Widget on Startup"
                description="Automatically display the desktop widget whenever Asao starts. Independent from 'Start Asao with Windows'."
                checked={settings.launchWidgetOnStartup}
                disabled={!settings.widgetEnabled}
                onChange={(val) =>
                  void updateSettingsPatch({ launchWidgetOnStartup: val })
                }
              />

              <ToggleRow
                title="Always on Top"
                description="Keep the desktop widget floating above standard application windows."
                checked={settings.alwaysOnTop}
                disabled={!settings.widgetEnabled}
                onChange={(val) =>
                  void updateSettingsPatch({ alwaysOnTop: val })
                }
              />

              <ToggleRow
                title="Show Recommendations"
                description="Surface live process resource insights and memory notices inside the widget."
                checked={settings.showRecommendations}
                disabled={!settings.widgetEnabled}
                onChange={(val) =>
                  void updateSettingsPatch({ showRecommendations: val })
                }
              />

              <ToggleRow
                title="Notifications"
                description="Allow Asao to send non-intrusive notifications for sustained high resource usage."
                checked={settings.notifications}
                disabled={!settings.widgetEnabled}
                onChange={(val) =>
                  void updateSettingsPatch({ notifications: val })
                }
              />
            </div>
          </section>

          {/* AI Diagnostics Assistant (Vertex AI ADK) Section */}
          <section className="rounded-xl lunar-glass-card p-5">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
                <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
                  AI Diagnostics Assistant (Vertex AI ADK)
                </h2>
              </div>
              <button
                type="button"
                onClick={() => setRoute("assistant")}
                className="px-2.5 py-1 rounded-md bg-lunar-surface-2 hover:bg-lunar-elevated border border-lunar-border text-xs text-lunar-white flex items-center gap-1.5 transition-colors cursor-pointer"
              >
                <span>Open Assistant</span>
                <ArrowUpRight className="w-3 h-3 text-lunar-ai" />
              </button>
            </div>

            <div className="space-y-4 pt-1 text-xs">
              <div className="grid grid-cols-2 gap-3">
                <div className="p-3 rounded-lg bg-lunar-bg border border-lunar-border/60">
                  <div className="text-lunar-muted text-[10px] uppercase tracking-wider flex items-center gap-1">
                    <Layers className="w-3 h-3 text-lunar-ai" />
                    <span>Target Model</span>
                  </div>
                  <div className="font-mono text-lunar-white font-medium mt-1">
                    gemini-3.5-flash-lite
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-lunar-bg border border-lunar-border/60">
                  <div className="text-lunar-muted text-[10px] uppercase tracking-wider flex items-center gap-1">
                    <Globe className="w-3 h-3 text-lunar-text-sec" />
                    <span>Vertex Location</span>
                  </div>
                  <div className="font-mono text-lunar-white font-medium mt-1">
                    global
                  </div>
                </div>
              </div>

              <div className="space-y-1.5">
                <label className="flex items-center justify-between text-lunar-white font-medium">
                  <span>Google Cloud Project ID</span>
                  <span className="text-[10px] text-lunar-muted">Configurable by you</span>
                </label>
                <input
                  type="text"
                  value={aiProjectId}
                  onChange={(e) => setAiProjectId(e.target.value)}
                  placeholder="e.g. your-gcp-project-id"
                  className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text placeholder:text-lunar-muted font-mono text-[11px] focus:outline-none focus:border-lunar-text-sec"
                />
              </div>

              <div className="space-y-1.5">
                <label className="flex items-center justify-between text-lunar-white font-medium">
                  <span className="flex items-center gap-1.5">
                    <Key className="w-3.5 h-3.5 text-lunar-text-sec" />
                    <span>API Key / Bearer Token</span>
                  </span>
                  <span className="text-[10px] text-lunar-muted">Optional</span>
                </label>
                <input
                  type="password"
                  value={aiApiKey}
                  onChange={(e) => setAiApiKey(e.target.value)}
                  placeholder="AIzaSy... or OAuth Access Token"
                  className="w-full px-3 py-2 rounded-lg bg-lunar-bg border border-lunar-border text-lunar-text placeholder:text-lunar-muted font-mono text-[11px] focus:outline-none focus:border-lunar-text-sec"
                />
              </div>

              <div className="flex items-center justify-between pt-1">
                <p className="text-[11px] text-lunar-muted">
                  When not configured, ASAO uses its local diagnostic engine.
                </p>

                <button
                  type="button"
                  onClick={() => {
                    updateAgentConfig({
                      projectId: aiProjectId.trim(),
                      apiKey: aiApiKey.trim(),
                    });
                    setAiSaved(true);
                    setTimeout(() => setAiSaved(false), 1500);
                  }}
                  className="px-3.5 py-1.5 rounded-lg bg-lunar-white text-lunar-bg font-medium text-xs hover:bg-lunar-text transition-colors flex items-center gap-1.5 cursor-pointer shadow-sm"
                >
                  {aiSaved && <Check className="w-3.5 h-3.5" />}
                  <span>{aiSaved ? "Saved" : "Save AI Config"}</span>
                </button>
              </div>
            </div>
          </section>
      </div>
    </div>
  );
};
