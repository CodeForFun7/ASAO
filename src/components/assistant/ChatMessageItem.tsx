import React, { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { ChatMessage } from "../../types/agent";
import { AssistantMessageView } from "./AssistantMessageView";

interface ChatMessageItemProps {
  message: ChatMessage;
  onApplyFix: (fixId: string, messageId: string) => void;
}

export const ChatMessageItem: React.FC<ChatMessageItemProps> = ({
  message,
  onApplyFix,
}) => {
  const isUser = message.sender === "user";
  const [isExpanded, setIsExpanded] = useState(false);

  if (isUser) {
    const isLongText = message.text.length > 220 || (message.text.match(/\n/g) || []).length > 3;

    return (
      <div className="flex justify-end my-3 pl-8 sm:pl-16">
        <div className="relative max-w-[85%] sm:max-w-[75%] rounded-2xl rounded-tr-sm px-4 py-3 bg-lunar-elevated/65 backdrop-blur-sm border border-lunar-border/80 text-[13px] text-lunar-white shadow-sm leading-relaxed">
          <div
            className={`whitespace-pre-wrap ${
              isLongText && !isExpanded ? "max-h-[115px] overflow-hidden" : ""
            }`}
          >
            {message.text}
          </div>

          {/* Fade overlay and Show More / Show Less button */}
          {isLongText && (
            <div
              className={`pt-1 ${
                !isExpanded
                  ? "absolute bottom-0 left-0 right-0 bg-gradient-to-t from-lunar-elevated via-lunar-elevated/90 to-transparent pt-6 pb-2 px-4 flex justify-start rounded-b-2xl"
                  : "mt-1 flex justify-start"
              }`}
            >
              <button
                type="button"
                onClick={() => setIsExpanded(!isExpanded)}
                className="text-[11px] font-medium text-zinc-400 hover:text-white flex items-center gap-1 transition-colors cursor-pointer select-none"
              >
                <span>{isExpanded ? "Show less" : "Show more"}</span>
                {isExpanded ? (
                  <ChevronUp className="w-3 h-3" />
                ) : (
                  <ChevronDown className="w-3 h-3" />
                )}
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="my-4">
      <AssistantMessageView message={message} onApplyFix={onApplyFix} />
    </div>
  );
};
