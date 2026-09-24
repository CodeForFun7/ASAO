import React from "react";
import { ArrowUpRight } from "lucide-react";

interface MetricCardProps {
  label: string;
  value: React.ReactNode;
  subtitle: React.ReactNode;
  statusDotColor?: string;
  onClick?: () => void;
  actionHint?: string;
}

export const MetricCard: React.FC<MetricCardProps> = ({
  label,
  value,
  subtitle,
  statusDotColor,
  onClick,
  actionHint,
}) => {
  return (
    <div
      onClick={onClick}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={
        onClick
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick();
              }
            }
          : undefined
      }
      className={`group relative rounded-lg bg-lunar-surface border border-lunar-border p-4 flex flex-col justify-between min-h-[124px] transition-colors ${
        onClick
          ? "hover:bg-lunar-surface-2 hover:border-lunar-text-sec/40 cursor-pointer"
          : ""
      }`}
    >
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec">
          {label}
        </span>
        {onClick && (
          <span
            className="text-lunar-muted group-hover:text-lunar-text transition-colors flex items-center gap-1 text-[10px] font-mono"
            title={actionHint}
          >
            <ArrowUpRight className="w-3.5 h-3.5" />
          </span>
        )}
      </div>

      <div className="my-2 flex items-baseline gap-2">
        {statusDotColor && (
          <span
            className={`w-2.5 h-2.5 rounded-full shrink-0 self-center ${statusDotColor}`}
          />
        )}
        <div className="text-2xl font-semibold tracking-tight text-lunar-white font-mono">
          {value}
        </div>
      </div>

      <div className="text-xs text-lunar-text-sec flex items-center gap-1.5 font-mono">
        {subtitle}
      </div>
    </div>
  );
};
