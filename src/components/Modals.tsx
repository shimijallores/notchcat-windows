import React, { useEffect, useState } from "react";
import { api } from "../api";

export const FocusDurationModal: React.FC<{
  initialMinutes: number;
  onClose: () => void;
}> = ({ initialMinutes, onClose }) => {
  const [custom, setCustom] = useState(String(initialMinutes));
  const presets = [15, 25, 45, 60];

  useEffect(() => {
    api.setModalOpen(true);
    return () => {
      api.setModalOpen(false);
    };
  }, []);

  const apply = (mins: number) => {
    const n = Math.max(1, Math.min(240, Math.round(mins)));
    api.timerSetDuration(n);
    onClose();
  };

  return (
    <div className="nc-modal-backdrop" onClick={onClose}>
      <div className="nc-modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="nc-modal-title">Focus Duration</div>
        <div className="nc-preset-row">
          {presets.map((m) => (
            <button
              key={m}
              className={`nc-preset-btn ${Number(custom) === m ? "active" : ""}`}
              onClick={() => apply(m)}
            >
              {m}m
            </button>
          ))}
        </div>
        <form
          className="nc-modal-form"
          onSubmit={(e) => {
            e.preventDefault();
            const parsed = parseInt(custom, 10);
            if (!Number.isNaN(parsed) && parsed > 0) apply(parsed);
          }}
        >
          <input
            type="number"
            min={1}
            max={240}
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
            className="nc-modal-input"
            autoFocus
            placeholder="Minutes (1–240)"
          />
          <button type="submit" className="nc-modal-submit">
            Set
          </button>
        </form>
      </div>
    </div>
  );
};

export const CustomReminderModal: React.FC<{
  taskTitle: string;
  initialTimestamp: number | null;
  onSave: (timestampSecs: number) => void;
  onClear: () => void;
  onClose: () => void;
}> = ({ taskTitle, initialTimestamp, onSave, onClear, onClose }) => {
  const baseDate = initialTimestamp
    ? new Date(initialTimestamp * 1000)
    : new Date(Date.now() + 3600_000);
  const pad = (n: number) => String(n).padStart(2, "0");
  const initLocal = `${baseDate.getFullYear()}-${pad(baseDate.getMonth() + 1)}-${pad(baseDate.getDate())}T${pad(baseDate.getHours())}:${pad(baseDate.getMinutes())}`;

  const [dtValue, setDtValue] = useState(initLocal);

  useEffect(() => {
    api.setModalOpen(true);
    return () => {
      api.setModalOpen(false);
    };
  }, []);

  return (
    <div className="nc-modal-backdrop" onClick={onClose}>
      <div className="nc-modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="nc-modal-title">Remind Me — {taskTitle}</div>
        <form
          className="nc-modal-col"
          onSubmit={(e) => {
            e.preventDefault();
            const ms = new Date(dtValue).getTime();
            if (!Number.isNaN(ms)) {
              onSave(Math.floor(ms / 1000));
              onClose();
            }
          }}
        >
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
                type="button"
                className="nc-modal-secondary"
                onClick={() => {
                  onClear();
                  onClose();
                }}
              >
                Clear
              </button>
            )}
            <button
              type="button"
              className="nc-modal-secondary"
              onClick={onClose}
            >
              Cancel
            </button>
            <button type="submit" className="nc-modal-submit">
              Save Reminder
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export const IcsUrlModal: React.FC<{
  initialUrl: string;
  onSave: (url: string) => void;
  onClose: () => void;
}> = ({ initialUrl, onSave, onClose }) => {
  const [url, setUrl] = useState(initialUrl);

  useEffect(() => {
    api.setModalOpen(true);
    return () => {
      api.setModalOpen(false);
    };
  }, []);

  return (
    <div className="nc-modal-backdrop" onClick={onClose}>
      <div className="nc-modal-card wide" onClick={(e) => e.stopPropagation()}>
        <div className="nc-modal-title">Calendar ICS Subscription URL</div>
        <p className="nc-modal-hint">
          Paste a read-only iCalendar (.ics / webcal://) URL from Google
          Calendar, iCloud, or Outlook.
        </p>
        <form
          className="nc-modal-col"
          onSubmit={(e) => {
            e.preventDefault();
            onSave(url.trim());
            onClose();
          }}
        >
          <input
            type="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            className="nc-modal-input"
            placeholder="https://calendar.google.com/calendar/ical/.../basic.ics"
            autoFocus
          />
          <div className="nc-modal-actions">
            <button
              type="button"
              className="nc-modal-secondary"
              onClick={onClose}
            >
              Cancel
            </button>
            <button type="submit" className="nc-modal-submit">
              Connect Calendar
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
