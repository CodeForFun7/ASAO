import React, { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useWidgetStore } from "../../stores/widget-store";
import { WidgetHeader } from "./WidgetHeader";
import { SystemCondition } from "./SystemCondition";
import { SystemMetrics } from "./SystemMetrics";
import { RecommendationCard } from "./RecommendationCard";
import { openMainWindow } from "../../services/widget";
import { getEffectiveTheme } from "../../services/theme";

type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

export const AsaoWidget: React.FC = () => {
  const settings = useWidgetStore((s) => s.settings);
  const telemetry = useWidgetStore((s) => s.telemetry);
  const history = useWidgetStore((s) => s.history);

  const initializeWidget = useWidgetStore((s) => s.initializeWidget);
  const updateSettingsPatch = useWidgetStore((s) => s.updateSettingsPatch);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;

    void initializeWidget().then((unlisten) => {
      if (mounted) {
        cleanup = unlisten;
      } else {
        unlisten();
      }
    });

    return () => {
      mounted = false;
      if (cleanup) cleanup();
    };
  }, [initializeWidget]);

  const startResize = (dir: ResizeDirection) => (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      void getCurrentWindow().startResizeDragging(dir);
    } catch {
      // Ignore in browser preview
    }
  };

  const opacityAlpha = Math.max(0.6, Math.min(1, settings.widgetOpacity / 100));
  const isLight = getEffectiveTheme(settings.theme || "dark") === "light";
  const bgRgba = isLight
    ? `rgba(255, 255, 255, ${opacityAlpha})`
    : `rgba(14, 14, 14, ${opacityAlpha})`;
  const condition = telemetry?.condition ?? "GOOD";
  const recommendationsList =
    telemetry?.recommendations && telemetry.recommendations.length > 0
      ? telemetry.recommendations
      : telemetry?.recommendation
      ? [telemetry.recommendation]
      : [];

  return (
    <div
      style={{
        backgroundColor: bgRgba,
      }}
      className="relative h-screen w-screen rounded-xl border border-lunar-border flex flex-col overflow-hidden select-none backdrop-blur-md"
    >
      {/* Invisible edge/corner resize handles let the user resize the widget. */}
      <div
        onMouseDown={startResize("North")}
        className="absolute top-0 left-2 right-2 h-1.5 cursor-n-resize z-40"
      />
      <div
        onMouseDown={startResize("South")}
        className="absolute bottom-0 left-2 right-2 h-1.5 cursor-s-resize z-40"
      />
      <div
        onMouseDown={startResize("West")}
        className="absolute left-0 top-2 bottom-2 w-1.5 cursor-w-resize z-40"
      />
      <div
        onMouseDown={startResize("East")}
        className="absolute right-0 top-2 bottom-2 w-1.5 cursor-e-resize z-40"
      />
      <div
        onMouseDown={startResize("NorthWest")}
        className="absolute top-0 left-0 w-3 h-3 cursor-nw-resize z-50"
      />
      <div
        onMouseDown={startResize("NorthEast")}
        className="absolute top-0 right-0 w-3 h-3 cursor-ne-resize z-50"
      />
      <div
        onMouseDown={startResize("SouthWest")}
        className="absolute bottom-0 left-0 w-3 h-3 cursor-sw-resize z-50"
      />
      <div
        onMouseDown={startResize("SouthEast")}
        className="absolute bottom-0 right-0 w-3 h-3 cursor-se-resize z-50"
      />

      {/* Draggable Header */}
      <WidgetHeader />

        <div className="flex-1 min-h-0 flex flex-col justify-between p-3.5 space-y-2.5 overflow-hidden">
          {/* Strain Indicator */}
          <SystemCondition
            condition={condition}
            reason={
              telemetry?.conditionReason ??
              "Collecting real-time system telemetry..."
            }
          />

          {/* CPU & RAM Metrics */}
          <SystemMetrics
            cpuUsage={telemetry?.cpuUsage ?? 0}
            memoryUsage={telemetry?.memoryUsage ?? 0}
            memoryUsedBytes={telemetry?.memoryUsedBytes ?? 0}
            memoryTotalBytes={telemetry?.memoryTotalBytes ?? 0}
            history={history}
          />

          {/* Auto-Scrolling Recommendations Card with Notification Count Badge */}
          <RecommendationCard recommendations={recommendationsList} />

          {/* Bottom Process Count Bar */}
          <div
            data-tauri-drag-region
            className="pt-1.5 border-t border-lunar-border/70 flex items-center justify-between text-xs text-lunar-text-sec shrink-0"
          >
            <button
              type="button"
              onClick={() => void openMainWindow("processes")}
              className="hover:text-lunar-white transition-colors cursor-pointer"
            >
              {telemetry?.processCount ?? 0} processes running
            </button>
          </div>
        </div>
    </div>
  );
};
