import React from "react";
import { Music2 } from "lucide-react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type { AppSnapshot } from "../types";
import { formatClock } from "../useNotchCat";
import { ReminderButtons } from "./HeaderBar";
import { DotMatrixText, pillPitch, pillWidth } from "./Visuals";

const ISLAND_W = 116;
const SHAPE_H = 22;
const GAP = 8;
const CHIP_W = 46;

const EqualizerBars: React.FC<{ playing: boolean }> = ({ playing }) => (
  <span className={`nc-eq ${playing ? "playing" : ""}`} aria-hidden="true">
    <i />
    <i />
    <i />
  </span>
);

export const CollapsedIsland: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const { ui, timer, music, settings } = snapshot;
  const style = settings.fakeNotch;
  const top = style === "notch" ? 0 : 3;

  if (ui.compact) {
    const c = ui.compact;
    const title = c.kind === "timesUp" ? "Time's up" : c.title;
    const subtitle =
      c.kind === "timesUp" ? (c.taskTitle ?? "Session complete") : "Reminder";
    return (
      <div className="nc-compact">
        <div className="nc-compact-row">
          <img src={mascotUrl} alt="" width={34} height={34} />
          <div className="nc-compact-text">
            <b>{title}</b>
            <span>{subtitle}</span>
          </div>
          <span style={{ flex: 1 }} />
          {c.kind === "timesUp" ? (
            <div className="nc-reminder-buttons">
              <button
                className="nc-snooze"
                onClick={() => {
                  api.timerAddTime(300);
                  api.dismissCompact();
                }}
              >
                <span>+5 min</span>
              </button>
              <button
                className="nc-done"
                onClick={() => {
                  api.timerReset();
                  api.dismissCompact();
                }}
              >
                <span>Done</span>
              </button>
            </div>
          ) : (
            <ReminderButtons
              snoozeMinutes={settings.snoozeMinutes}
              onSnooze={() => api.reminderSnooze(c.taskId)}
              onDone={() => api.reminderComplete(c.taskId)}
            />
          )}
        </div>
      </div>
    );
  }

  const showPill = settings.showPill && timer.state !== "idle";
  const track = music.track;
  const showChip =
    (settings.showMusicChip ||
      settings.cards.some((c) => c.card === "music" && c.enabled)) &&
    track !== null &&
    track.playing;

  const handleOpen = () => {
    if (settings.clickToOpen || style === "island") api.openTray(true);
  };

  const clock = formatClock(timer.remaining);
  const pw = pillWidth(clock, SHAPE_H);
  const finished = timer.state === "finished";
  const paused = timer.state === "paused";

  // Offsets are relative to the screen's horizontal center.
  const pillLeft = style === "off" ? -pw / 2 : ISLAND_W / 2 + GAP;
  const chipRight =
    style === "off"
      ? showPill
        ? -pw / 2 - GAP
        : CHIP_W / 2
      : -ISLAND_W / 2 - GAP;

  return (
    <div className="nc-collapsed" style={{ top }}>
      {style !== "off" && (
        <button
          className={`nc-shape ${style}`}
          onClick={handleOpen}
          aria-label="Open NotchCat"
          title="NotchCat — hover or press Alt+N"
        />
      )}

      {showChip && track && (
        <button
          className="nc-chip"
          style={{ left: chipRight - CHIP_W }}
          onClick={handleOpen}
          title={`${track.title} — ${track.artist}`}
        >
          <span className="nc-chip-art">
            {track.artworkDataUrl ? (
              <img src={track.artworkDataUrl} alt="" />
            ) : (
              <Music2 size={9} />
            )}
          </span>
          <EqualizerBars playing={track.playing} />
        </button>
      )}

      {showPill && (
        <button
          className="nc-pill"
          style={{ left: pillLeft, width: pw }}
          onClick={handleOpen}
          title="Focus timer"
        >
          <span
            className={finished ? "nc-blink" : undefined}
            style={{ opacity: paused ? 0.55 : 1, display: "flex" }}
          >
            <DotMatrixText
              text={clock}
              pitch={pillPitch(SHAPE_H)}
              lit={finished ? "var(--amber)" : "#fff"}
              unlitOpacity={0.09}
            />
          </span>
        </button>
      )}
    </div>
  );
};
