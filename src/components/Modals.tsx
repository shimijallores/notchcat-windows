import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Minus, Plus } from "lucide-react";
import { api } from "../api";

/** Dims the housing and centers a small raised popover, like ModalHost on macOS. */
const ModalShell: React.FC<{
  onClose: () => void;
  wide?: boolean;
  children: React.ReactNode;
}> = ({ onClose, wide, children }) => {
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    api.setModalOpen(true);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onCloseRef.current();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      api.setModalOpen(false);
      window.removeEventListener("keydown", onKey, true);
    };
  }, []);

  const host = document.querySelector(".nc-tray") ?? document.body;
  return createPortal(
    <div className="nc-modal-backdrop" onPointerDown={onClose}>
      <div
        className={`nc-modal-card ${wide ? "wide" : ""}`}
        onPointerDown={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>,
    host,
  );
};

export const FocusDurationModal: React.FC<{
  initialMinutes: number;
  allowsClear?: boolean;
  onSave?: (minutes: number) => void;
  onClear?: () => void;
  onClose: () => void;
}> = ({ initialMinutes, allowsClear, onSave, onClear, onClose }) => {
  const [minutes, setMinutes] = useState(
    Math.min(Math.max(initialMinutes, 1), 240),
  );
  const presets = [15, 25, 45, 60];

  const save = () => {
    if (onSave) onSave(minutes);
    else api.timerSetDuration(minutes);
    onClose();
  };

  return (
    <ModalShell onClose={onClose}>
      <div className="nc-modal-title">Focus duration</div>
      <div className="nc-preset-row">
        {presets.map((m) => (
          <button
            key={m}
            className={`nc-preset-btn ${minutes === m ? "active" : ""}`}
            onClick={() => setMinutes(m)}
          >
            {m}m
          </button>
        ))}
      </div>
      <div className="nc-modal-custom">
        <span>Custom</span>
        <div className="nc-stepper">
          <button onClick={() => setMinutes((v) => Math.max(1, v - 5))} aria-label="Less">
            <Minus size={11} strokeWidth={3} />
          </button>
          <span className="nc-stepper-value">{minutes} min</span>
          <button onClick={() => setMinutes((v) => Math.min(240, v + 5))} aria-label="More">
            <Plus size={11} strokeWidth={3} />
          </button>
        </div>
      </div>
      <div className="nc-modal-actions">
        {allowsClear && (
          <button
            className="nc-modal-link"
            onClick={() => {
              onClear?.();
              onClose();
            }}
          >
            Remove limit
          </button>
        )}
        <span style={{ flex: 1 }} />
        <button className="nc-modal-secondary" onClick={onClose}>
          Cancel
        </button>
        <button className="nc-modal-submit" onClick={save} autoFocus>
          Save
        </button>
      </div>
    </ModalShell>
  );
};

export const CustomReminderModal: React.FC<{
  taskTitle: string;
  initialTimestamp: number | null;
  onSave: (timestampSecs: number) => void;
  onClear: () => void;
  onClose: () => void;
}> = ({ initialTimestamp, onSave, onClear, onClose }) => {
  const baseDate = initialTimestamp && initialTimestamp * 1000 > Date.now()
    ? new Date(initialTimestamp * 1000)
    : new Date(Date.now() + 3600_000);
  const pad = (n: number) => String(n).padStart(2, "0");
  const initLocal = `${baseDate.getFullYear()}-${pad(baseDate.getMonth() + 1)}-${pad(baseDate.getDate())}T${pad(baseDate.getHours())}:${pad(baseDate.getMinutes())}`;
  const [dtValue, setDtValue] = useState(initLocal);

  return (
    <ModalShell onClose={onClose}>
      <div className="nc-modal-title">Remind me</div>
      <input
        type="datetime-local"
        value={dtValue}
        onChange={(e) => setDtValue(e.target.value)}
        className="nc-modal-input"
        autoFocus
      />
      <div className="nc-modal-actions">
        {initialTimestamp !== null && (
          <button
            className="nc-modal-link"
            onClick={() => {
              onClear();
              onClose();
            }}
          >
            Clear
          </button>
        )}
        <span style={{ flex: 1 }} />
        <button className="nc-modal-secondary" onClick={onClose}>
          Cancel
        </button>
        <button
          className="nc-modal-submit"
          onClick={() => {
            const ms = new Date(dtValue).getTime();
            if (!Number.isNaN(ms)) {
              onSave(Math.floor(ms / 1000));
              onClose();
            }
          }}
        >
          Set reminder
        </button>
      </div>
    </ModalShell>
  );
};

export const IcsUrlModal: React.FC<{
  initialUrl: string;
  onSave: (url: string) => void;
  onClose: () => void;
}> = ({ initialUrl, onSave, onClose }) => {
  const [url, setUrl] = useState(initialUrl);
  const save = () => {
    onSave(url.trim());
    onClose();
  };

  return (
    <ModalShell onClose={onClose} wide>
      <div className="nc-modal-title">Calendar feed</div>
      <p className="nc-modal-hint">
        Paste a read-only iCalendar (.ics or webcal://) link from Google
        Calendar, Outlook or iCloud.
      </p>
      <input
        type="url"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && save()}
        className="nc-modal-input"
        placeholder="https://calendar.google.com/calendar/ical/.../basic.ics"
        autoFocus
      />
      <div className="nc-modal-actions">
        {initialUrl && (
          <button
            className="nc-modal-link"
            onClick={() => {
              onSave("");
              onClose();
            }}
          >
            Disconnect
          </button>
        )}
        <span style={{ flex: 1 }} />
        <button className="nc-modal-secondary" onClick={onClose}>
          Cancel
        </button>
        <button className="nc-modal-submit" onClick={save}>
          Save
        </button>
      </div>
    </ModalShell>
  );
};
