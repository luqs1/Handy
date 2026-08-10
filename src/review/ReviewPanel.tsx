import React, { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import "./ReviewPanel.css";

interface ReviewShowPayload {
  text: string;
}

/**
 * Editable review panel shown between transcription and paste.
 *
 * Keys: Enter pastes the (possibly edited) text, a bare Option/Alt tap pastes
 * immediately without further edits, Escape dismisses without pasting,
 * Shift+Enter inserts a newline.
 */
const ReviewPanel: React.FC = () => {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [edited, setEdited] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // True while an Option/Alt press has not been followed by any other key —
  // releasing Alt then counts as a "tap" and confirms as-is. Option-as-a-
  // modifier (⌥→ word jumps, ⌥-letter special characters) clears the flag on
  // the second keydown, so it never misfires during editing.
  const altTapPending = useRef(false);
  // The submit/cancel round-trip is fast, but guard against double-fires
  // (e.g. Alt-tap racing Enter).
  const decided = useRef(false);

  useEffect(() => {
    const unlisten = listen<ReviewShowPayload>("review-show", (event) => {
      decided.current = false;
      altTapPending.current = false;
      setEdited(false);
      setText(event.payload.text);
      requestAnimationFrame(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          el.setSelectionRange(el.value.length, el.value.length);
        }
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const submit = useCallback((value: string) => {
    if (decided.current) return;
    decided.current = true;
    invoke("review_submit", { text: value }).catch(() => {
      decided.current = false;
    });
  }, []);

  const cancel = useCallback(() => {
    if (decided.current) return;
    decided.current = true;
    invoke("review_cancel").catch(() => {
      decided.current = false;
    });
  }, []);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Alt") {
      altTapPending.current = true;
      return;
    }
    altTapPending.current = false;

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit(event.currentTarget.value);
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancel();
    }
  };

  const handleKeyUp = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Alt" && altTapPending.current) {
      altTapPending.current = false;
      submit(event.currentTarget.value);
    }
  };

  return (
    <div className="review-panel">
      <div className="review-header">
        <span className="review-title">{t("review.title")}</span>
        {edited && <span className="review-edited">{t("review.edited")}</span>}
      </div>
      <textarea
        ref={textareaRef}
        className="review-textarea"
        value={text}
        spellCheck={false}
        onChange={(event) => {
          setText(event.target.value);
          setEdited(true);
        }}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
      />
      <div className="review-hints">
        <span>
          <kbd>{t("review.keyEnter")}</kbd> {t("review.hintPaste")}
        </span>
        <span>
          <kbd>{t("review.keyOption")}</kbd> {t("review.hintAsIs")}
        </span>
        <span>
          <kbd>{t("review.keyEscape")}</kbd> {t("review.hintDismiss")}
        </span>
      </div>
    </div>
  );
};

export default ReviewPanel;
