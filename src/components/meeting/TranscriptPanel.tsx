import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { User, Bot } from "lucide-react";
import { useMeetingsStore } from "@/stores/meetingStore";
import type { Utterance } from "@/bindings";

interface TranscriptPanelProps {
  sessionId?: string | null;
}

export const TranscriptPanel: React.FC<TranscriptPanelProps> = ({
  sessionId,
}) => {
  const { t } = useTranslation();
  const { utterances, isActive, loadTranscript } = useMeetingsStore();
  const [pastTranscript, setPastTranscript] = useState<Utterance[]>([]);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [utterances]);

  useEffect(() => {
    if (sessionId && !isActive) {
      loadTranscript(sessionId).then(setPastTranscript);
    } else {
      setPastTranscript([]);
    }
  }, [sessionId, isActive, loadTranscript]);

  const displayUtterances = isActive ? utterances : pastTranscript;

  if (displayUtterances.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-mid-gray text-sm">
        {isActive
          ? t("meeting.transcript.waiting")
          : t("meeting.transcript.empty")}
      </div>
    );
  }

  const formatTime = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-3">
      {displayUtterances.map((utt) => (
        <div
          key={utt.id}
          className={`flex gap-3 ${
            utt.speaker === "you" ? "flex-row-reverse" : ""
          }`}
        >
          <div
            className={`shrink-0 w-8 h-8 rounded-full flex items-center justify-center ${
              utt.speaker === "you"
                ? "bg-logo-primary/20 text-logo-primary"
                : "bg-mid-gray/20 text-mid-gray"
            }`}
          >
            {utt.speaker === "you" ? <User size={16} /> : <Bot size={16} />}
          </div>
          <div
            className={`max-w-[80%] rounded-lg p-3 ${
              utt.speaker === "you"
                ? "bg-logo-primary/10"
                : "bg-mid-gray/10"
            }`}
          >
            <div className="text-xs text-mid-gray mb-1">
              {utt.speaker === "you" ? t("meeting.you") : t("meeting.them")} ·{" "}
              {formatTime(utt.timestamp_ms)}
            </div>
            <p className="text-sm">{utt.text}</p>
          </div>
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
};
