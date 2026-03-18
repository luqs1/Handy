import React from "react";
import { useTranslation } from "react-i18next";
import { Mic, MicOff, Square } from "lucide-react";
import { useMeetingsStore } from "@/stores/meetingStore";

export const MeetingControlBar: React.FC = () => {
  const { t } = useTranslation();
  const {
    isActive,
    isRecording,
    startMeeting,
    stopMeeting,
    micLevel,
    systemLevel,
  } = useMeetingsStore();

  const handleToggle = () => {
    if (isActive) {
      stopMeeting();
    } else {
      startMeeting();
    }
  };

  return (
    <div className="w-full max-w-md mx-auto p-4 bg-mid-gray/10 rounded-xl border border-mid-gray/20">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold">{t("meeting.title")}</h2>
        <button
          onClick={handleToggle}
          className={`flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-all ${
            isActive
              ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
              : "bg-logo-primary/80 text-white hover:bg-logo-primary"
          }`}
        >
          {isActive ? (
            <>
              <Square size={16} />
              {t("meeting.stop")}
            </>
          ) : (
            <>
              <Mic size={16} />
              {t("meeting.start")}
            </>
          )}
        </button>
      </div>

      {isActive && (
        <div className="space-y-3">
          <div>
            <div className="flex justify-between text-xs text-mid-gray mb-1">
              <span>{t("meeting.mic")}</span>
              <span>{Math.round(micLevel * 100)}%</span>
            </div>
            <div className="h-2 bg-mid-gray/20 rounded-full overflow-hidden">
              <div
                className="h-full bg-green-500 transition-all"
                style={{ width: `${Math.min(micLevel * 100, 100)}%` }}
              />
            </div>
          </div>
          <div>
            <div className="flex justify-between text-xs text-mid-gray mb-1">
              <span>{t("meeting.systemAudio")}</span>
              <span>{Math.round(systemLevel * 100)}%</span>
            </div>
            <div className="h-2 bg-mid-gray/20 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 transition-all"
                style={{ width: `${Math.min(systemLevel * 100, 100)}%` }}
              />
            </div>
          </div>
          {isRecording && (
            <div className="flex items-center gap-2 text-xs text-mid-gray">
              <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
              {t("meeting.recording")}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
