import React from "react";
import { Sparkles } from "lucide-react";
import { askNyra } from "../../services/ask-nyra";

interface AskNyraButtonProps {
  /** Builds the message sent to Nyra. Called on click so it reads the entity's current details. */
  buildPrompt: () => string;
  /** `icon` is a compact square button for dense rows. */
  variant?: "full" | "icon";
  className?: string;
}

export const AskNyraButton: React.FC<AskNyraButtonProps> = ({ buildPrompt, variant = "full", className = "" }) => {
  const onClick = (event: React.MouseEvent) => {
    event.stopPropagation();
    askNyra(buildPrompt());
  };

  if (variant === "icon") {
    return (
      <button
        type="button"
        onClick={onClick}
        title="Ask Nyra about this"
        aria-label="Ask Nyra about this"
        className={`inline-flex items-center justify-center w-7 h-7 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-lunar-ai border border-lunar-border transition-colors cursor-pointer ${className}`}
      >
        <Sparkles className="w-3 h-3" />
      </button>
    );
  }

  return (
    <button
      type="button"
      onClick={onClick}
      title="Send the details of this item to Nyra and get a plain-language explanation"
      className={`inline-flex items-center justify-center gap-2 px-3 py-2 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-xs font-medium text-lunar-ai border border-lunar-border transition-colors cursor-pointer ${className}`}
    >
      <Sparkles className="w-3.5 h-3.5" />
      <span>Ask Nyra</span>
    </button>
  );
};
