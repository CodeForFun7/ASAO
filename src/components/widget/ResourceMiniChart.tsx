import React, { useMemo } from "react";

interface ResourceMiniChartProps {
  values: number[];
  strokeColor: string;
  gradientId: string;
}

export const ResourceMiniChart: React.FC<ResourceMiniChartProps> = ({
  values,
  strokeColor,
  gradientId,
}) => {
  const { polylineStr, areaStr } = useMemo(() => {
    const count = 20;
    const fallback = values[0] ?? 0;
    const padded =
      values.length < count
        ? [...Array(count - values.length).fill(fallback), ...values]
        : values.slice(values.length - count);

    const width = 120;
    const height = 26;

    const pts = padded.map((val, idx) => {
      const x = (idx / (count - 1)) * width;
      const clamped = Math.max(0, Math.min(100, val));
      const y = height - (clamped / 100) * (height - 6) - 3;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });

    return {
      polylineStr: pts.join(" "),
      areaStr: `M 0,${height} L ${pts.join(" L ")} L ${width},${height} Z`,
    };
  }, [values]);

  return (
    <div className="h-[26px] w-full pt-1">
      <svg
        viewBox="0 0 120 26"
        preserveAspectRatio="none"
        className="w-full h-full overflow-visible"
      >
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={strokeColor} stopOpacity="0.22" />
            <stop offset="100%" stopColor={strokeColor} stopOpacity="0.0" />
          </linearGradient>
        </defs>
        <path d={areaStr} fill={`url(#${gradientId})`} />
        <polyline
          fill="none"
          stroke={strokeColor}
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          points={polylineStr}
        />
      </svg>
    </div>
  );
};
