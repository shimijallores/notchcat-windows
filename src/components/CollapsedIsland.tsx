import React from "react";
import { api } from "../api";
import type { AppSnapshot } from "../types";
import { formatClock } from "../useNotchCat";
import { DotMatrixDigits } from "./Visuals";

export const CollapsedIsland: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const { ui, timer, music, settings } = snapshot;
  const style = settings.fakeNotch;

  const showPill =
    settings.showPill &&
    (timer.state !== "idle");
  const showChip =
    (settings.showMusicChip ||
      settings.cards.some((c) => c.card === "music" && c.enabled)) &&
    music.track !== null &&
    music.track.playing;

  const handleOpen = () => {
    if (settings.clickToOpen || style === "island") {
      api.openTray(true);
    }
  };

  if (ui.compact) {
    return (
      <div
        className={`nc-compact-wrap ${style === "notch" ? "is-notch" : "is-island"}`}
      >
        {ui.compact.kind === "timesUp" ? (
          <div className="nc-compact-banner">
            <div className="nc-compact-left">
              <span className="nc-compact-badge">
                <DotMatrixDigits text="00:00" dotSize={1.5} gap={0.6} />
              </span>
              <div className="nc-compact-text">
                <span className="nc-compact-title">Time&apos;s up</span>
                {ui.compact.taskTitle && (
                  <span className="nc-compact-sub">{ui.compact.taskTitle}</span>
                )}
              </div>
            </div>
            <div className="nc-compact-actions">
              <button
                className="nc-compact-btn"
                onClick={() => api.timerAddTime(60)}
              >
                +1m
              </button>
              <button
                className="nc-compact-btn"
                onClick={() => api.timerAddTime(300)}
              >
                +5m
              </button>
              <button
                className="nc-compact-btn primary"
                onClick={() => api.timerReset()}
              >
                Done
              </button>
            </div>
          </div>
        ) : (
          <div className="nc-compact-banner">
            <div className="nc-compact-left">
              <span className="nc-reminder-dot" />
              <div className="nc-compact-text">
                <span className="nc-compact-title">{ui.compact.title}</span>
                <span className="nc-compact-sub">Reminder due</span>
              </div>
            </div>
            <div className="nc-compact-actions">
              <button
                className="nc-compact-btn primary"
                onClick={() => api.reminderComplete(ui.compact!.kind === "reminder" ? ui.compact!.taskId : "")}
              >
                Done
              </button>
              <button
                className="nc-compact-btn"
                onClick={() => api.reminderSnooze(ui.compact!.kind === "reminder" ? ui.compact!.taskId : "")}
              >
                Snooze {settings.snoozeMinutes}m
              </button>
              <button
                className="nc-compact-btn"
                onClick={() => api.reminderDismiss()}
              >
                ×
              </button>
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      className={`nc-collapsed-bar ${style === "notch" ? "is-notch" : "is-island"}`}
    >
      {/* Left Music Chip */}
      <div className="nc-collapsed-slot left">
        {showChip && music.track && (
          <button
            className="nc-music-chip"
            onClick={handleOpen}
            title={`${music.track.title} — ${music.track.artist}`}
          >
            <span className="nc-eq-bars" aria-hidden="true">
              <i />
              <i />
              <i />
            </span>
            <span className="nc-music-chip-title">{music.track.title}</span>
          </button>
        )}
      </div>

      {/* Center Top Island / Notch Shape */}
      <div className="nc-collapsed-slot center">
        {style !== "off" && (
          <button
            className={`nc-island-pill ${style === "notch" ? "notch-shape" : "island-shape"}`}
            onClick={handleOpen}
            aria-label="Open NotchCat"
            title="NotchCat — Hover or Alt+N to open"
          >
            <span className="nc-island-camera-dot" />
          </button>
        )}
      </div>

      {/* Right Dot-Matrix Timer Pill */}
      <div className="nc-collapsed-slot right">
        {showPill && (
          <button
            className={`nc-timer-pill ${timer.state}`}
            onClick={handleOpen}
            title="Focus timer active"
          >
            <DotMatrixDigits
              text={formatClock(timer.remaining)}
              dotSize={1.45}
              gap={0.55}
              color={
                timer.state === "paused"
                  ? "rgba(252,193,80,0.62)"
                  : timer.state === "finished"
                    ? "#ff7b7b"
                    : "var(--amber)"
              }
              blinkColon={timer.state === "running"}
            />
          </button>
        )}
      </div>
    </div>
  );
};
