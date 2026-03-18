import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Calendar, Clock, MessageSquare, ChevronRight } from "lucide-react";
import { useMeetingsStore } from "@/stores/meetingStore";
import { MeetingControlBar } from "./MeetingControlBar";
import { TranscriptPanel } from "./TranscriptPanel";
import { NotesPanel } from "./NotesPanel";
import type { MeetingSessionSummary } from "@/bindings";

type Tab = "transcript" | "notes";

export const MeetingView: React.FC = () => {
  const { t } = useTranslation();
  const {
    isActive,
    currentSessionId,
    pastMeetings,
    currentMeeting,
  } = useMeetingsStore();
  const [activeTab, setActiveTab] = useState<Tab>("transcript");
  const [selectedSession, setSelectedSession] = useState<string | null>(null);

  const displaySessionId = isActive ? currentSessionId : (selectedSession ?? currentMeeting?.id ?? null);

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  };

  const formatDuration = (secs: number) => {
    const mins = Math.floor(secs / 60);
    return `${mins} min`;
  };

  return (
    <div className="w-full max-w-3xl mx-auto p-6 space-y-6">
      <MeetingControlBar />

      {!isActive && pastMeetings.length > 0 && (
        <div className="bg-mid-gray/5 rounded-xl border border-mid-gray/20 p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <Calendar size={14} />
            {t("meeting.history.title")}
          </h3>
          <div className="space-y-2 max-h-48 overflow-y-auto">
            {pastMeetings.slice(0, 10).map((meeting) => (
              <button
                key={meeting.id}
                onClick={() => setSelectedSession(meeting.id)}
                className={`w-full text-left p-3 rounded-lg border transition-colors ${
                  selectedSession === meeting.id
                    ? "border-logo-primary/50 bg-logo-primary/5"
                    : "border-transparent hover:bg-mid-gray/10"
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <Clock size={14} className="text-mid-gray" />
                    <div>
                      <div className="text-sm font-medium">
                        {formatDate(meeting.started_at)}
                      </div>
                      <div className="text-xs text-mid-gray flex items-center gap-2">
                        <span>{formatDuration(meeting.duration_secs)}</span>
                        <span>·</span>
                        <span>
                          {meeting.utterance_count}{" "}
                          {t("meeting.history.utterances")}
                        </span>
                      </div>
                    </div>
                  </div>
                  <ChevronRight size={14} className="text-mid-gray" />
                </div>
              </button>
            ))}
          </div>
        </div>
      )}

      {(isActive || displaySessionId) && (
        <div className="bg-mid-gray/5 rounded-xl border border-mid-gray/20 overflow-hidden">
          <div className="flex border-b border-mid-gray/20">
            <button
              onClick={() => setActiveTab("transcript")}
              className={`flex-1 px-4 py-3 text-sm font-medium flex items-center justify-center gap-2 transition-colors ${
                activeTab === "transcript"
                  ? "border-b-2 border-logo-primary text-logo-primary"
                  : "text-mid-gray hover:text-foreground"
              }`}
            >
              <MessageSquare size={14} />
              {t("meeting.tabs.transcript")}
            </button>
            <button
              onClick={() => setActiveTab("notes")}
              className={`flex-1 px-4 py-3 text-sm font-medium flex items-center justify-center gap-2 transition-colors ${
                activeTab === "notes"
                  ? "border-b-2 border-logo-primary text-logo-primary"
                  : "text-mid-gray hover:text-foreground"
              }`}
            >
              {t("meeting.tabs.notes")}
            </button>
          </div>

          {activeTab === "transcript" ? (
            <TranscriptPanel sessionId={displaySessionId} />
          ) : (
            <NotesPanel sessionId={displaySessionId} />
          )}
        </div>
      )}
    </div>
  );
};
