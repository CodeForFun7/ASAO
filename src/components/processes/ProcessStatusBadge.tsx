import React from "react";
import { Lock } from "lucide-react";
import { STATUS_METADATA, type ProcessStatus } from "../../types/process";

interface ProcessStatusBadgeProps {
  status: ProcessStatus;
  compact?: boolean;
}

export const ProcessStatusBadge: React.FC<ProcessStatusBadgeProps> = ({
  status,
  compact = false,
}) => {
  const meta = STATUS_METADATA[status] ?? STATUS_METADATA.normal;

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded border px-2 py-0.5 text-[11px] font-mono ${meta.badgeBg} ${meta.badgeText} ${meta.badgeBorder}`}
    >
      {status === "protected" ? (
        <Lock className="w-2.5 h-2.5 shrink-0" />
      ) : (
        <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${meta.dotColor}`} />
      )}
      <span className="truncate">
        {compact && status === "high-resource" ? "High Resource" : meta.label}
      </span>
    </span>
  );
};
