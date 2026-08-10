import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ReviewBeforePasteProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ReviewBeforePaste: React.FC<ReviewBeforePasteProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("review_before_paste") ?? false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) => updateSetting("review_before_paste", enabled)}
        isUpdating={isUpdating("review_before_paste")}
        label={t("settings.debug.reviewBeforePaste.label")}
        description={t("settings.debug.reviewBeforePaste.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
