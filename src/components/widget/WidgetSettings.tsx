import React, { useEffect, useState } from "react";
import {
  AppWindow,
  Check,
  Eye,
  EyeOff,
  Sliders,
  Sun,
  Moon,
  Monitor,
  Palette,
  PackageX,
  X,
} from "lucide-react";
import { useWidgetStore } from "../../stores/widget-store";
import { hideWidget } from "../../services/widget";
import {
  getIgnoreList,
  getResidualInfo,
  unignoreApp,
  unignoreFolder,
} from "../../services/residual";
import { unignoreUnusedApp } from "../../services/apps";
import type { IgnoreList, ResidualInfo } from "../../types/residual";

const OPACITY_PRESETS = [60, 70, 75, 80, 90, 100];
const LEFTOVER_AGE_PRESETS = [7, 14, 30, 60, 90];

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
  const [ignoreList, setIgnoreList] = useState<IgnoreList | null>(null);
  const [residualInfo, setResidualInfo] = useState<ResidualInfo | null>(null);

  useEffect(() => {
    void getIgnoreList().then(setIgnoreList).catch(() => setIgnoreList(null));
    void getResidualInfo().then(setResidualInfo).catch(() => setResidualInfo(null));
  }, []);

  const agePresets = residualInfo?.devMode ? [0, ...LEFTOVER_AGE_PRESETS] : LEFTOVER_AGE_PRESETS;
  const ignoredCount =
    (ignoreList?.folders.length ?? 0) + (ignoreList?.apps.length ?? 0) + (ignoreList?.unusedApps?.length ?? 0);

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
        {/* Appearance & Theme Section */}
        <section className="rounded-xl lunar-glass-card p-5">
          <div className="flex items-center gap-2 mb-3">
            <Palette className="w-3.5 h-3.5 text-lunar-text-sec" />
            <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
              Appearance &amp; Theme
            </h2>
          </div>

          <p className="text-[11px] text-lunar-text-sec mb-4 leading-relaxed">
            Choose your preferred color theme or synchronize dynamically with your Windows system preference.
          </p>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {/* Dark Theme Option */}
            <button
              type="button"
              onClick={() => void updateSettingsPatch({ theme: "dark" })}
              className={`relative p-3.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between ${
                (settings.theme || "dark") === "dark"
                  ? "bg-lunar-elevated border-lunar-white/80 shadow-sm"
                  : "bg-lunar-surface-2/60 border-lunar-border hover:bg-lunar-surface-2 hover:border-lunar-text-sec/40"
              }`}
            >
              <div className="flex items-center justify-between mb-3">
                <div className="w-8 h-8 rounded-lg bg-[#141414] border border-[#2A2A2A] flex items-center justify-center text-white shrink-0">
                  <Moon className="w-4 h-4" />
                </div>
                {(settings.theme || "dark") === "dark" && (
                  <span className="w-4 h-4 rounded-full bg-lunar-white text-lunar-bg flex items-center justify-center text-[10px]">
                    <Check className="w-2.5 h-2.5 stroke-[3]" />
                  </span>
                )}
              </div>

              <div>
                <div className="text-xs font-semibold text-lunar-white">Dark Theme</div>
                <div className="text-[10px] text-lunar-text-sec mt-0.5 leading-tight">
                  Deep lunar black palette for low-light focus
                </div>
              </div>
            </button>

            {/* Light Theme Option */}
            <button
              type="button"
              onClick={() => void updateSettingsPatch({ theme: "light" })}
              className={`relative p-3.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between ${
                settings.theme === "light"
                  ? "bg-lunar-elevated border-lunar-white/80 shadow-sm"
                  : "bg-lunar-surface-2/60 border-lunar-border hover:bg-lunar-surface-2 hover:border-lunar-text-sec/40"
              }`}
            >
              <div className="flex items-center justify-between mb-3">
                <div className="w-8 h-8 rounded-lg bg-white border border-[#E2E4E8] flex items-center justify-center text-[#0D1117] shrink-0">
                  <Sun className="w-4 h-4" />
                </div>
                {settings.theme === "light" && (
                  <span className="w-4 h-4 rounded-full bg-lunar-white text-lunar-bg flex items-center justify-center text-[10px]">
                    <Check className="w-2.5 h-2.5 stroke-[3]" />
                  </span>
                )}
              </div>

              <div>
                <div className="text-xs font-semibold text-lunar-white">Light Theme</div>
                <div className="text-[10px] text-lunar-text-sec mt-0.5 leading-tight">
                  Clean, crisp high-contrast palette for daytime
                </div>
              </div>
            </button>

            {/* System Option */}
            <button
              type="button"
              onClick={() => void updateSettingsPatch({ theme: "system" })}
              className={`relative p-3.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between ${
                settings.theme === "system"
                  ? "bg-lunar-elevated border-lunar-white/80 shadow-sm"
                  : "bg-lunar-surface-2/60 border-lunar-border hover:bg-lunar-surface-2 hover:border-lunar-text-sec/40"
              }`}
            >
              <div className="flex items-center justify-between mb-3">
                <div className="w-8 h-8 rounded-lg bg-lunar-surface border border-lunar-border flex items-center justify-center text-lunar-white shrink-0">
                  <Monitor className="w-4 h-4" />
                </div>
                {settings.theme === "system" && (
                  <span className="w-4 h-4 rounded-full bg-lunar-white text-lunar-bg flex items-center justify-center text-[10px]">
                    <Check className="w-2.5 h-2.5 stroke-[3]" />
                  </span>
                )}
              </div>

              <div>
                <div className="text-xs font-semibold text-lunar-white">System (Auto)</div>
                <div className="text-[10px] text-lunar-text-sec mt-0.5 leading-tight">
                  Synchronize automatically with Windows appearance
                </div>
              </div>
            </button>
          </div>
        </section>

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

          {/* Leftover Cleanup Section */}
          <section className="rounded-xl lunar-glass-card p-5">
            <div className="flex items-center gap-2 mb-3">
              <PackageX className="w-3.5 h-3.5 text-lunar-text-sec" />
              <h2 className="text-xs font-semibold uppercase tracking-wider text-lunar-white">
                Leftover Cleanup
              </h2>
            </div>

            <div className="py-3.5 border-b border-lunar-border/60 space-y-2.5">
              <div>
                <div className="text-xs font-medium text-lunar-white">Skip recently used folders</div>
                <div className="text-[11px] text-lunar-text-sec mt-0.5">
                  A folder with any file changed in this many days is never shown as a leftover.
                </div>
              </div>
              <div className="flex items-center gap-1.5">
                {agePresets.map((days) => (
                  <button
                    key={days}
                    type="button"
                    onClick={() => void updateSettingsPatch({ residualMinAgeDays: days })}
                    title={days === 0 ? "Developer override: also show folders changed today" : undefined}
                    className={`flex-1 py-1 rounded border text-[11px] transition-colors cursor-pointer ${
                      (settings.residualMinAgeDays ?? 30) === days
                        ? "bg-lunar-elevated border-lunar-text-sec text-lunar-white"
                        : "bg-lunar-bg border-lunar-border text-lunar-muted hover:text-lunar-text-sec"
                    }`}
                  >
                    {days === 0 ? "0 days (dev)" : `${days} days`}
                  </button>
                ))}
              </div>
            </div>

            <div className="py-3.5 space-y-2.5">
              <div>
                <div className="text-xs font-medium text-lunar-white">Ignored items</div>
                <div className="text-[11px] text-lunar-text-sec mt-0.5">
                  Folders and apps you chose not to see again. Remove one to have scans show it again.
                </div>
              </div>

              {ignoredCount === 0 ? (
                <div className="text-[11px] text-lunar-muted py-2">
                  Nothing is ignored. Use "Don't show again" on the Leftovers or Unused apps tabs in Storage.
                </div>
              ) : (
                <div className="space-y-1.5">
                  {ignoreList?.apps.map((a) => (
                    <div
                      key={`app-${a.key}`}
                      className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg bg-lunar-surface-2/60 border border-lunar-border"
                    >
                      <div className="min-w-0">
                        <div className="text-xs text-lunar-white truncate">{a.name}</div>
                        <div className="text-[10px] text-lunar-muted">App, all its folders</div>
                      </div>
                      <button
                        type="button"
                        onClick={() => void unignoreApp(a.key).then(setIgnoreList)}
                        className="inline-flex items-center gap-1 px-2 py-1 rounded text-[11px] text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer shrink-0"
                      >
                        <X className="w-3 h-3" />
                        <span>Remove</span>
                      </button>
                    </div>
                  ))}
                  {ignoreList?.unusedApps?.map((a) => (
                    <div
                      key={`unused-${a.key}`}
                      className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg bg-lunar-surface-2/60 border border-lunar-border"
                    >
                      <div className="min-w-0">
                        <div className="text-xs text-lunar-white truncate">{a.name}</div>
                        <div className="text-[10px] text-lunar-muted">Not suggested on Unused apps</div>
                      </div>
                      <button
                        type="button"
                        onClick={() => void unignoreUnusedApp(a.key).then(setIgnoreList)}
                        className="inline-flex items-center gap-1 px-2 py-1 rounded text-[11px] text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer shrink-0"
                      >
                        <X className="w-3 h-3" />
                        <span>Remove</span>
                      </button>
                    </div>
                  ))}
                  {ignoreList?.folders.map((f) => (
                    <div
                      key={`folder-${f.path}`}
                      className="flex items-center justify-between gap-3 px-3 py-2 rounded-lg bg-lunar-surface-2/60 border border-lunar-border"
                    >
                      <div className="min-w-0">
                        <div className="font-mono text-[10px] text-lunar-text truncate" title={f.path}>
                          {f.path}
                        </div>
                        <div className="text-[10px] text-lunar-muted">Folder</div>
                      </div>
                      <button
                        type="button"
                        onClick={() => void unignoreFolder(f.path).then(setIgnoreList)}
                        className="inline-flex items-center gap-1 px-2 py-1 rounded text-[11px] text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer shrink-0"
                      >
                        <X className="w-3 h-3" />
                        <span>Remove</span>
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {residualInfo && (
              <div className="pt-3 border-t border-lunar-border/60 text-[10px] text-lunar-muted space-y-0.5">
                <div>
                  Deletion log: <span className="font-mono break-all">{residualInfo.deletionLog}</span>
                </div>
                <div>
                  Dry-run log: <span className="font-mono break-all">{residualInfo.dryRunLog}</span>
                </div>
              </div>
            )}
          </section>

      </div>
    </div>
  );
};
