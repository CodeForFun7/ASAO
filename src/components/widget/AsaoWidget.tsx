import React, { useEffect } from "react";
import { useWidgetStore } from "../../stores/widget-store";
import { WidgetHeader } from "./WidgetHeader";
import { SystemCondition } from "./SystemCondition";
import { SystemMetrics } from "./SystemMetrics";
import { RecommendationCard } from "./RecommendationCard";
import { WidgetChat } from "./WidgetChat";
import { openMainWindow } from "../../services/widget";

export const AsaoWidget: React.FC = () => {
  const mode = useWidgetStore((s) => s.mode);
  const settings = useWidgetStore((s) => s.settings);
  const telemetry = useWidgetStore((s) => s.telemetry);
  const history = useWidgetStore((s) => s.history);
  const messages = useWidgetStore((s) => s.messages);
  const isSendingChat = useWidgetStore((s) => s.isSendingChat);

  const initializeWidget = useWidgetStore((s) => s.initializeWidget);
  const switchMode = useWidgetStore((s) => s.switchMode);
  const updateSettingsPatch = useWidgetStore((s) => s.updateSettingsPatch);
  const sendChatPrompt = useWidgetStore((s) => s.sendChatPrompt);

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

  const opacityAlpha = Math.max(0.6, Math.min(1, settings.widgetOpacity / 100));
  const condition = telemetry?.condition ?? "GOOD";

  return (
    <div className="h-screen w-screen p-1.5 bg-transparent overflow-hidden select-none">
      <div
        style={{
          backgroundColor: `rgba(16, 19, 24, ${opacityAlpha})`,
        }}
        className="h-full w-full rounded-xl border border-lunar-border shadow-2xl flex flex-col overflow-hidden backdrop-blur-md"
      >
        {/* Draggable Header */}
        <WidgetHeader
          mode={mode}
          condition={condition}
          settings={settings}
          onBackToMonitor={() => void switchMode("monitor")}
          onUpdateSettings={(patch) => void updateSettingsPatch(patch)}
        />

        {/* Monitor Mode vs Chat Mode */}
        {mode === "chat" ? (
          <WidgetChat
            telemetry={telemetry}
            messages={messages}
            isSending={isSendingChat}
            onSendMessage={(text) => void sendChatPrompt(text)}
          />
        ) : (
          <div className="flex-1 flex flex-col justify-between p-3 space-y-2.5 overflow-hidden">
            {/* System Condition */}
            <SystemCondition
              condition={condition}
              reason={
                telemetry?.conditionReason ??
                "Collecting real-time system telemetry..."
              }
            />

            {/* CPU & RAM Metrics + Activity Curves */}
            <SystemMetrics
              cpuUsage={telemetry?.cpuUsage ?? 0}
              memoryUsage={telemetry?.memoryUsage ?? 0}
              memoryUsedBytes={telemetry?.memoryUsedBytes ?? 0}
              memoryTotalBytes={telemetry?.memoryTotalBytes ?? 0}
              history={history}
            />

            {/* Recommendation & Ask Asao */}
            {telemetry && (
              <RecommendationCard
                recommendation={telemetry.recommendation}
                showRecommendations={settings.showRecommendations}
                onAskAsao={(question) => void switchMode("chat", question)}
              />
            )}

            {/* Bottom Process Count Bar */}
            <div
              data-tauri-drag-region
              className="pt-1 border-t border-lunar-border/70 flex items-center justify-between text-[11px] text-lunar-text-sec"
            >
              <button
                type="button"
                onClick={() => void openMainWindow("processes")}
                className="hover:text-lunar-white transition-colors cursor-pointer"
              >
                {telemetry?.processCount ?? 0} processes running
              </button>
              <span className="text-[10px] text-lunar-muted">
                {telemetry?.attentionCount ?? 0} attention
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
