import React, { useEffect } from "react";
import {
  AppWindow,
  Eye,
  EyeOff,
  Sliders,
  ShieldCheck,
  Database,
  Sparkles,
} from "lucide-react";
import { useWidgetStore } from "../../stores/widget-store";
import { hideWidget, showWidget } from "../../services/widget";
import type { WidgetPositionPreset } from "../../types/widget";

const POSITION_OPTIONS: Array<{ value: WidgetPositionPreset; label: string }> = [
  { value: "top-left", label: "Top Left" },
  { value: "top-right", label: "Top Right" },
  { value: "bottom-left", label: "Bottom Left" },
  { value: "bottom-right", label: "Bottom Right" },
];

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
  const initializeWidget = useWidgetStore((s) => s.initializeWidget);
  const updateSettingsPatch = useWidgetStore((s) => s.updateSettingsPatch);

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
      {/* Page Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold text-lunar-white tracking-tight">
            Settings
          </h1>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            Configure ASAO system startup, desktop companion widget, and telemetry preferences.
          </p>
        </div>

        {/* Direct Widget Show / Hide Action Buttons */}
        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={!settings.widgetEnabled}
            onClick={() => void showWidget()}
            className="h-8 px-3 rounded-md bg-lunar-elevated hover:bg-lunar-border disabled:opacity-40 text-xs font-medium text-lunar-white border border-lunar-border flex items-center gap-1.5 transition-colors cursor-pointer"
          >
            <Eye className="w-3.5 h-3.5" />
            <span>Show Widget</span>
          </button>

          <button
            type="button"
            onClick={() => void hideWidget()}
            className="h-8 px-3 rounded-md bg-lunar-surface hover:bg-lunar-surface-2 text-xs text-lunar-text-sec hover:text-lunar-white border border-lunar-border flex items-center gap-1.5 transition-colors cursor-pointer"
          >
            <EyeOff className="w-3.5 h-3.5" />
            <span>Hide Widget</span>
          </button>
        </div>
      </div>

      {/* Main Settings Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left 7 Cols: Desktop Widget & Startup Controls */}
        <div className="lg:col-span-7 space-y-6">
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
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                <AppWindow className="w-3.5 h-3.5 text-lunar-white" />
                <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
                  Desktop Widget
                </h2>
              </div>
              <span className="text-[10px] px-2 py-0.5 rounded bg-lunar-bg border border-lunar-border text-lunar-text-sec">
                {settings.widgetEnabled ? "ENABLED" : "DISABLED"}
              </span>
            </div>

            <div>
              <ToggleRow
                title="Desktop Widget"
                description="Enable the companion desktop widget and the 'Show Widget' system tray control."
                checked={settings.widgetEnabled}
                onChange={(val) => void handleToggleWidgetEnabled(val)}
              />

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
        </div>

        {/* Right 5 Cols: Position, Opacity & Architecture Status */}
        <div className="lg:col-span-5 space-y-6">
          {/* Position & Opacity */}
          <section
            className={`rounded-xl lunar-glass-card p-5 space-y-5 ${
              !settings.widgetEnabled ? "opacity-45 pointer-events-none" : ""
            }`}
          >
            <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
              Appearance &amp; Placement
            </h2>

            {/* Widget Position Selector */}
            <div className="space-y-2">
              <div className="flex items-center justify-between text-xs">
                <span className="text-lunar-text-sec">Widget Position</span>
                <span className="text-lunar-white capitalize">
                  {settings.widgetPosition.replace("-", " ")}
                </span>
              </div>

              <div className="grid grid-cols-2 gap-2">
                {POSITION_OPTIONS.map((pos) => {
                  const active = settings.widgetPosition === pos.value;
                  return (
                    <button
                      key={pos.value}
                      type="button"
                      onClick={() =>
                        void updateSettingsPatch({
                          widgetPosition: pos.value,
                        })
                      }
                      className={`py-2 px-3 rounded-md border text-xs font-medium transition-colors cursor-pointer ${
                        active
                          ? "bg-lunar-elevated border-lunar-text-sec text-lunar-white"
                          : "bg-lunar-bg border-lunar-border text-lunar-text-sec hover:text-lunar-white"
                      }`}
                    >
                      {pos.label}
                    </button>
                  );
                })}
              </div>
              <p className="text-[11px] text-lunar-muted">
                You can also drag the widget anywhere on screen; its custom coordinates are saved automatically.
              </p>
            </div>

            {/* Widget Opacity Control */}
            <div className="space-y-2.5 border-t border-lunar-border pt-4">
              <div className="flex items-center justify-between text-xs">
                <span className="text-lunar-text-sec">Widget Opacity</span>
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
          </section>

          {/* Subsystem Readiness Cards */}
          <section className="rounded-xl lunar-glass-card p-5 space-y-3">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-lunar-white">
              <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
              <span>AI &amp; Persistence Boundaries</span>
            </div>

            <div className="p-3 rounded bg-lunar-bg border border-lunar-border space-y-1.5 text-xs">
              <div className="flex items-center gap-1.5 text-lunar-ai font-medium">
                <Database className="w-3.5 h-3.5" />
                <span>SystemContextService &amp; ChatService</span>
              </div>
              <p className="text-lunar-text-sec text-[11px] leading-relaxed">
                Widget chat packages structured CPU, RAM, top processes, and active recommendations without exposing raw process tables.
              </p>
            </div>

            <div className="p-3 rounded bg-lunar-bg border border-lunar-border space-y-1.5 text-xs">
              <div className="flex items-center gap-1.5 text-lunar-healthy font-medium">
                <ShieldCheck className="w-3.5 h-3.5" />
                <span>Read-Only Observation Policy</span>
              </div>
              <p className="text-lunar-text-sec text-[11px] leading-relaxed">
                Closing the widget hides it without terminating Asao. Destructive process or registry actions remain disabled.
              </p>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
};
