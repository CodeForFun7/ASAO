import React, { useState } from "react";
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  Info,
  Play,
  RotateCcw,
  Square,
  X,
  Zap,
} from "lucide-react";
import type { WprStatus } from "../../types/startup";

interface WprBootTraceModalProps {
  status: WprStatus | null;
  isOpen: boolean;
  onClose: () => void;
  onStartTrace: () => Promise<string>;
  onCancelTrace: () => Promise<string>;
}

export const WprBootTraceModal: React.FC<WprBootTraceModalProps> = ({
  status,
  isOpen,
  onClose,
  onStartTrace,
  onCancelTrace,
}) => {
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [isError, setIsError] = useState(false);

  if (!isOpen) return null;

  const handleStart = async () => {
    setLoading(true);
    setFeedback(null);
    try {
      const msg = await onStartTrace();
      setFeedback(msg);
      setIsError(false);
    } catch (err) {
      setFeedback(err instanceof Error ? err.message : String(err));
      setIsError(true);
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = async () => {
    setLoading(true);
    setFeedback(null);
    try {
      const msg = await onCancelTrace();
      setFeedback(msg);
      setIsError(false);
    } catch (err) {
      setFeedback(err instanceof Error ? err.message : String(err));
      setIsError(true);
    } finally {
      setLoading(false);
    }
  };

  const isAvailable = status?.isAvailable ?? false;
  const isConfigured = status?.isBootConfigured ?? false;
  const isRecording = status?.isRecording ?? false;

  return (
    <div className="fixed inset-0 z-50 bg-black/75 backdrop-blur-xs flex items-center justify-center p-4">
      <div className="w-full max-w-xl bg-lunar-surface border border-lunar-border rounded-xl shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        {/* Modal Header */}
        <div className="p-4 border-b border-lunar-border flex items-center justify-between bg-lunar-surface-2/60">
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-lunar-elevated border border-lunar-border flex items-center justify-center">
              <Activity className="w-3.5 h-3.5 text-lunar-ai" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-lunar-white">
                Windows Performance Recorder (WPR) & ETW
              </h2>
              <p className="text-[11px] text-lunar-text-sec">
                Hardware boot tracing & timeline impact measurement
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded hover:bg-lunar-elevated text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Modal Content */}
        <div className="p-5 space-y-4 max-h-[80vh] overflow-y-auto">
          {/* Status Banner */}
          <div className="p-3.5 rounded-lg border border-lunar-border bg-lunar-bg/70 flex items-start gap-3">
            {isRecording ? (
              <Zap className="w-4 h-4 text-lunar-warning mt-0.5 shrink-0" />
            ) : isConfigured ? (
              <CheckCircle2 className="w-4 h-4 text-lunar-healthy mt-0.5 shrink-0" />
            ) : isAvailable ? (
              <Info className="w-4 h-4 text-lunar-ai mt-0.5 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 text-lunar-critical mt-0.5 shrink-0" />
            )}
            <div className="min-w-0 flex-1">
              <div className="text-xs font-semibold text-lunar-white">
                {isRecording
                  ? "Active ETW Trace Recording"
                  : isConfigured
                  ? "Boot Trace Configured for Next Restart"
                  : isAvailable
                  ? "WPR Installed & Ready"
                  : "WPR Not Detected"}
              </div>
              <div className="text-[11px] text-lunar-text-sec mt-0.5">
                {status?.message ??
                  "Checking Windows Performance Recorder subsystem..."}
              </div>
            </div>
          </div>

          {/* Feedback message if any */}
          {feedback && (
            <div
              className={`p-3 rounded-lg border text-xs font-mono leading-relaxed ${
                isError
                  ? "bg-lunar-critical/10 text-lunar-critical border-lunar-critical/30"
                  : "bg-lunar-healthy/10 text-lunar-healthy border-lunar-healthy/30"
              }`}
            >
              {feedback}
            </div>
          )}

          {/* Educational / Explanatory Section */}
          <div className="space-y-3 text-xs text-lunar-text leading-relaxed">
            <h3 className="font-semibold text-lunar-white flex items-center gap-1.5">
              <Zap className="w-3.5 h-3.5 text-lunar-warning" />
              <span>How Asao Measures Accurate Boot Impact</span>
            </h3>

            <p className="text-lunar-text-sec text-[11px]">
              Raw CPU time alone is not an accurate indicator of boot delay. A
              process can run on background CPU cores without slowing down the
              user experience. Conversely, high disk I/O queueing and registry
              locks during the <strong className="text-lunar-text">PostLogon</strong> and{" "}
              <strong className="text-lunar-text">Desktop Ready</strong> phases block the
              Windows shell and defer user interactivity.
            </p>

            <div className="p-3 rounded-lg bg-lunar-surface-2/40 border border-lunar-border/80 space-y-2 text-[11px]">
              <div className="font-medium text-lunar-white">
                ETW Boot Tracing Process:
              </div>
              <ul className="list-disc pl-4 space-y-1 text-lunar-text-sec">
                <li>
                  <strong className="text-lunar-text">Configure Autologger:</strong> WPR sets up
                  an ETW kernel session that starts immediately when Windows boots.
                </li>
                <li>
                  <strong className="text-lunar-text">Restart PC:</strong> The system logs kernel
                  driver init, service startup, and interactive logon timeline.
                </li>
                <li>
                  <strong className="text-lunar-text">Timeline Correlation:</strong> Asao
                  correlates discovered startup entries with exact disk, thread, and
                  memory footprints recorded during boot.
                </li>
              </ul>
            </div>
          </div>

          {/* Actions */}
          <div className="pt-3 border-t border-lunar-border/60 flex items-center justify-between gap-3">
            <div className="text-[10px] text-lunar-muted font-mono">
              Note: Boot trace configuration requires Administrator privileges.
            </div>

            <div className="flex items-center gap-2 shrink-0">
              {isConfigured && (
                <button
                  type="button"
                  disabled={loading}
                  onClick={() => void handleCancel()}
                  className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-lunar-surface-2 hover:bg-lunar-critical/20 text-lunar-text hover:text-lunar-critical border border-lunar-border hover:border-lunar-critical/40 transition-colors cursor-pointer"
                >
                  <Square className="w-3 h-3" />
                  <span>Cancel Boot Trace</span>
                </button>
              )}

              <button
                type="button"
                disabled={loading || !isAvailable || isRecording}
                onClick={() => void handleStart()}
                className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded text-xs font-medium bg-lunar-white hover:bg-white/90 text-black transition-colors cursor-pointer disabled:opacity-50"
              >
                {loading ? (
                  <RotateCcw className="w-3.5 h-3.5 animate-spin text-black" />
                ) : (
                  <Play className="w-3.5 h-3.5 text-black" />
                )}
                <span>Configure Boot Trace</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
