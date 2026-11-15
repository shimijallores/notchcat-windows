import React, { useRef, useState } from "react";
import { api } from "../api";
import type {
  AppSnapshot,
  CardID,
  SettingsData,
  TaskItem,
} from "../types";
import { formatClock } from "../useNotchCat";
import { CustomReminderModal, FocusDurationModal, IcsUrlModal } from "./Modals";
import { PaperCard, SevenSegmentClock } from "./Visuals";

const CARD_WIDTHS: Record<CardID, number> = {
  tasks: 330,
  timer: 236,
  notepad: 220,
  events: 224,
  music: 236,
};

function formatTimeShort(tsMs: number): string {
  return new Date(tsMs).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

// ---------------------------------------------------------------------------
// 1. TasksCard
// ---------------------------------------------------------------------------

const TasksCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [dayTab, setDayTab] = useState<"today" | "tomorrow">("today");
  const [newTitle, setNewTitle] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [menuTaskId, setMenuTaskId] = useState<string | null>(null);
  const [reminderModalTask, setReminderModalTask] = useState<TaskItem | null>(
    null,
  );
  const [showDone, setShowDone] = useState(false);
  const [dragId, setDragId] = useState<string | null>(null);

  const dayTs =
    dayTab === "today"
      ? snapshot.todayStartSecs
      : snapshot.weekDays[1]?.day ?? snapshot.todayStartSecs + 86400;
  const allTasks =
    dayTab === "today" ? snapshot.todayTasks : snapshot.tomorrowTasks;
  const activeTasks = allTasks.filter((t) => !t.isDone);
  const doneTasks = allTasks.filter((t) => t.isDone);

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = newTitle.trim();
    if (!trimmed) return;
    api.addTask(trimmed, dayTs);
    setNewTitle("");
  };

  const handleReorderDrop = (targetId: string) => {
    if (!dragId || dragId === targetId) return;
    const ids = allTasks.map((t) => t.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return;
    ids.splice(from, 1);
    ids.splice(to, 0, dragId);
    api.reorderTasks(dayTs, ids);
    setDragId(null);
    api.setDragActive(false);
  };

  const renderRow = (task: TaskItem) => {
    const isLinked = snapshot.timer.linkedTaskId === task.id;
    const isEditing = editingId === task.id;
    const menuOpen = menuTaskId === task.id;

    return (
      <div
        key={task.id}
        className={`nc-task-row ${task.isDone ? "done" : ""} ${isLinked ? "linked" : ""}`}
        draggable={!task.isDone && !isEditing}
        onDragStart={(e) => {
          setDragId(task.id);
          api.setDragActive(true);
          e.dataTransfer.setData("text/plain", task.id);
        }}
        onDragEnd={() => {
          setDragId(null);
          api.setDragActive(false);
        }}
        onDragOver={(e) => e.preventDefault()}
        onDrop={() => handleReorderDrop(task.id)}
      >
        <button
          className={`nc-checkbox ${task.isDone ? "checked" : ""}`}
          onClick={() => api.toggleTask(task.id)}
          aria-label={task.isDone ? "Mark incomplete" : "Mark done"}
        >
          {task.isDone && "✓"}
        </button>

        {isEditing ? (
          <input
            className="nc-task-rename"
            value={editTitle}
            autoFocus
            onChange={(e) => setEditTitle(e.target.value)}
            onBlur={() => {
              if (editTitle.trim() && editTitle.trim() !== task.title) {
                api.renameTask(task.id, editTitle.trim());
              }
              setEditingId(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                if (editTitle.trim()) api.renameTask(task.id, editTitle.trim());
                setEditingId(null);
              } else if (e.key === "Escape") {
                e.stopPropagation();
                setEditingId(null);
              }
            }}
          />
        ) : (
          <div
            className="nc-task-main"
            onDoubleClick={() => {
              setEditingId(task.id);
              setEditTitle(task.title);
            }}
          >
            <span className="nc-task-title">{task.title}</span>
            <span className="nc-task-badges">
              {task.timeLimitMinutes && (
                <span className="nc-task-tag">{task.timeLimitMinutes}m</span>
              )}
              {task.remindAt && !task.reminded && (
                <span className="nc-task-tag bell">
                  🔔 {formatTimeShort(task.remindAt * 1000)}
                </span>
              )}
            </span>
          </div>
        )}

        <div className="nc-task-actions">
          {!task.isDone && (
            <button
              className={`nc-task-mini ${isLinked ? "active" : ""}`}
              title="Focus on this task"
              onClick={() =>
                api.timerStart(
                  task.timeLimitMinutes ?? snapshot.settings.defaultFocusMinutes,
                  task.id,
                )
              }
            >
              ▶
            </button>
          )}
          <div className="nc-popover-anchor">
            <button
              className="nc-task-mini"
              title="More actions"
              onClick={() =>
                setMenuTaskId(menuOpen ? null : task.id)
              }
            >
              ⋯
            </button>
            {menuOpen && (
              <div
                className="nc-menu-popover"
                onMouseLeave={() => setMenuTaskId(null)}
              >
                <button
                  onClick={() => {
                    setEditingId(task.id);
                    setEditTitle(task.title);
                    setMenuTaskId(null);
                  }}
                >
                  Rename
                </button>
                <button
                  onClick={() => {
                    const otherTs =
                      dayTab === "today"
                        ? snapshot.weekDays[1]?.day ?? snapshot.todayStartSecs + 86400
                        : snapshot.todayStartSecs;
                    api.moveTaskToDate(task.id, otherTs);
                    setMenuTaskId(null);
                  }}
                >
                  Move to {dayTab === "today" ? "Tomorrow" : "Today"}
                </button>
                <button
                  onClick={() => {
                    api.duplicateTask(task.id);
                    setMenuTaskId(null);
                  }}
                >
                  Duplicate
                </button>
                <div className="nc-menu-divider" />
                <div className="nc-menu-label">Remind me</div>
                {snapshot.reminderPresets.map((p) => (
                  <button
                    key={p.id}
                    onClick={() => {
                      api.setTaskReminder(task.id, p.timestamp);
                      setMenuTaskId(null);
                    }}
                  >
                    {p.title}
                  </button>
                ))}
                <button
                  onClick={() => {
                    setReminderModalTask(task);
                    setMenuTaskId(null);
                  }}
                >
                  Custom time…
                </button>
                {task.remindAt !== null && (
                  <button
                    onClick={() => {
                      api.setTaskReminder(task.id, null);
                      setMenuTaskId(null);
                    }}
                  >
                    Clear reminder
                  </button>
                )}
                <div className="nc-menu-divider" />
                <button
                  className="danger"
                  onClick={() => {
                    api.deleteTask(task.id);
                    setMenuTaskId(null);
                  }}
                >
                  Delete
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  };

  return (
    <>
      <PaperCard
        title="Tasks"
        bg="var(--mint)"
        width={CARD_WIDTHS.tasks}
        grain={grain}
        headerRight={
          <div className="nc-seg-pill">
            <button
              className={dayTab === "today" ? "active" : ""}
              onClick={() => setDayTab("today")}
            >
              Today ({snapshot.todayTasks.filter((t) => !t.isDone).length})
            </button>
            <button
              className={dayTab === "tomorrow" ? "active" : ""}
              onClick={() => setDayTab("tomorrow")}
            >
              Tomorrow
            </button>
          </div>
        }
      >
        <form onSubmit={handleAdd} className="nc-task-add-row">
          <input
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
            placeholder={
              dayTab === "today" ? "+ Add a task for today…" : "+ Plan for tomorrow…"
            }
            className="nc-task-input"
          />
        </form>

        <div className="nc-scroll-list">
          {activeTasks.length === 0 && doneTasks.length === 0 && (
            <div className="nc-empty-note">Nothing lined up yet.</div>
          )}
          {activeTasks.map(renderRow)}
          {doneTasks.length > 0 && (
            <div className="nc-done-group">
              <button
                className="nc-done-toggle"
                onClick={() => setShowDone((v) => !v)}
              >
                {showDone ? "▾" : "▸"} Completed ({doneTasks.length})
              </button>
              {showDone && doneTasks.map(renderRow)}
            </div>
          )}
        </div>
      </PaperCard>

      {reminderModalTask && (
        <CustomReminderModal
          taskTitle={reminderModalTask.title}
          initialTimestamp={reminderModalTask.remindAt}
          onSave={(ts) => api.setTaskReminder(reminderModalTask.id, ts)}
          onClear={() => api.setTaskReminder(reminderModalTask.id, null)}
          onClose={() => setReminderModalTask(null)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 2. TimerCard
// ---------------------------------------------------------------------------

const TimerCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [durationOpen, setDurationOpen] = useState(false);
  const { timer, todayTasks } = snapshot;
  const linkedTask = timer.linkedTaskId
    ? todayTasks.find((t) => t.id === timer.linkedTaskId)
    : null;
  const remainingMins = Math.max(1, Math.round(timer.sessionTotal / 60));

  return (
    <>
      <PaperCard
        title="Focus"
        bg="var(--lavender)"
        width={CARD_WIDTHS.timer}
        grain={grain}
        headerRight={
          <button
            className="nc-card-chip-btn"
            onClick={() => setDurationOpen(true)}
            title="Set duration"
          >
            {remainingMins}m
          </button>
        }
      >
        <div className="nc-timer-body">
          <div className="nc-timer-clock-wrap">
            <SevenSegmentClock
              text={formatClock(timer.remaining)}
              onClick={() => setDurationOpen(true)}
              title="Click to change focus duration"
            />
            <div className="nc-timer-progress-track">
              <div
                className="nc-timer-progress-fill"
                style={{ width: `${Math.round(timer.progress * 100)}%` }}
              />
            </div>
          </div>

          <div className="nc-timer-linked">
            {linkedTask ? (
              <span className="nc-linked-pill" title={linkedTask.title}>
                ● {linkedTask.title}
              </span>
            ) : (
              <span className="nc-timer-hint">
                {timer.state === "running"
                  ? "Stay with one thing"
                  : timer.state === "finished"
                    ? "Session complete"
                    : "Tap clock or hit Start"}
              </span>
            )}
          </div>

          <div className="nc-timer-controls">
            {timer.state === "finished" ? (
              <>
                <button
                  className="nc-ink-btn"
                  onClick={() => api.timerAddTime(300)}
                >
                  +5m
                </button>
                <button
                  className="nc-ink-btn primary"
                  onClick={() => api.timerReset()}
                >
                  Done
                </button>
              </>
            ) : (
              <>
                <button
                  className="nc-ink-btn primary"
                  onClick={() => api.timerToggle()}
                >
                  {timer.state === "running"
                    ? "Pause"
                    : timer.state === "paused"
                      ? "Resume"
                      : "Start"}
                </button>
                {timer.state !== "idle" ? (
                  <>
                    <button
                      className="nc-ink-btn"
                      onClick={() => api.timerAddTime(60)}
                      title="Add 1 minute"
                    >
                      +1m
                    </button>
                    <button
                      className="nc-ink-btn"
                      onClick={() => api.timerReset()}
                      title="Reset"
                    >
                      ↺
                    </button>
                  </>
                ) : (
                  <button
                    className="nc-ink-btn"
                    onClick={() => setDurationOpen(true)}
                  >
                    {remainingMins}m
                  </button>
                )}
              </>
            )}
          </div>
        </div>
      </PaperCard>

      {durationOpen && (
        <FocusDurationModal
          initialMinutes={remainingMins}
          onClose={() => setDurationOpen(false)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 3. NotepadCard
// ---------------------------------------------------------------------------

const NotepadCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [localText, setLocalText] = useState(snapshot.noteText);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const syncTimer = useRef<number | null>(null);

  // Sync external updates (e.g. from cloud sync or undo) when not actively out-of-sync
  React.useEffect(() => {
    setLocalText(snapshot.noteText);
  }, [snapshot.noteText]);

  const handleChange = (val: string) => {
    setLocalText(val);
    if (syncTimer.current) window.clearTimeout(syncTimer.current);
    syncTimer.current = window.setTimeout(() => {
      api.noteSetText(val);
    }, 220);
  };

  const handleConvert = async () => {
    const el = textareaRef.current;
    const cursor = el ? el.selectionStart : 0;
    if (syncTimer.current) {
      window.clearTimeout(syncTimer.current);
      await api.noteSetText(localText);
    }
    const res = await api.noteConvertLine(cursor);
    if (res.converted) {
      setLocalText(res.nextText);
      requestAnimationFrame(() => {
        if (textareaRef.current) {
          textareaRef.current.selectionStart = res.cursorUtf16;
          textareaRef.current.selectionEnd = res.cursorUtf16;
        }
      });
    }
  };

  return (
    <PaperCard
      title="Notepad"
      bg="var(--sand)"
      width={CARD_WIDTHS.notepad}
      grain={grain}
      headerRight={
        <div className="nc-row-gap">
          {snapshot.noteHasUndo && (
            <button
              className="nc-card-chip-btn"
              onClick={() => api.noteUndo()}
              title="Undo note action"
            >
              Undo
            </button>
          )}
          <span className="nc-word-count">{snapshot.noteWordCount}w</span>
        </div>
      }
    >
      <div className="nc-notepad-wrap">
        <textarea
          ref={textareaRef}
          className="nc-notepad-area"
          value={localText}
          onChange={(e) => handleChange(e.target.value)}
          onBlur={() => {
            if (localText !== snapshot.noteText) api.noteSetText(localText);
          }}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
              e.preventDefault();
              handleConvert();
            }
          }}
          placeholder="Scratchpad… Ctrl+Enter turns a line into a task"
        />
        <button
          className="nc-convert-btn"
          onClick={handleConvert}
          title="Turn current line into a Today task (Ctrl+Enter)"
        >
          ↳ Make task (Ctrl+Enter)
        </button>
      </div>
    </PaperCard>
  );
};

// ---------------------------------------------------------------------------
// 4. EventsCard
// ---------------------------------------------------------------------------

const EventsCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [icsOpen, setIcsOpen] = useState(false);
  const { calendar, todayReminders, settings } = snapshot;

  const saveIcs = (url: string) => {
    const next: SettingsData = {
      ...settings,
      icsUrl: url || null,
    };
    api.settingsUpdate(next);
  };

  return (
    <>
      <PaperCard
        title="Events"
        bg="var(--slate)"
        width={CARD_WIDTHS.events}
        grain={grain}
        headerRight={
          <button
            className="nc-card-chip-btn"
            onClick={() => setIcsOpen(true)}
            title="Configure ICS Calendar URL"
          >
            {settings.icsUrl ? "Feed" : "+ Feed"}
          </button>
        }
      >
        <div className="nc-scroll-list">
          {calendar.access.kind === "failed" && (
            <div className="nc-empty-note error">
              Feed error: {calendar.access.message}
            </div>
          )}
          {calendar.access.kind === "notDetermined" && (
            <button
              className="nc-feed-setup-btn"
              onClick={() => setIcsOpen(true)}
            >
              Connect ICS calendar…
            </button>
          )}
          {calendar.events.map((ev) => (
            <div key={ev.id} className="nc-event-row">
              <span
                className="nc-event-bar"
                style={{
                  background: `rgb(${ev.color[0]},${ev.color[1]},${ev.color[2]})`,
                }}
              />
              <div className="nc-event-info">
                <div className="nc-event-title">{ev.title}</div>
                <div className="nc-event-time">
                  {ev.isAllDay
                    ? "All day"
                    : `${formatTimeShort(ev.startMs)} – ${formatTimeShort(ev.endMs)}`}
                </div>
              </div>
            </div>
          ))}

          {settings.showRemindersInEvents && todayReminders.length > 0 && (
            <div className="nc-events-reminders">
              <div className="nc-subhead">Reminders today</div>
              {todayReminders.map((t) => (
                <div key={t.id} className="nc-event-row">
                  <span className="nc-event-bar reminder" />
                  <div className="nc-event-info">
                    <div className="nc-event-title">{t.title}</div>
                    <div className="nc-event-time">
                      {t.remindAt ? formatTimeShort(t.remindAt * 1000) : ""}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}

          {calendar.access.kind === "authorized" &&
            calendar.events.length === 0 &&
            (!settings.showRemindersInEvents || todayReminders.length === 0) && (
              <div className="nc-empty-note">Clear schedule today.</div>
            )}
        </div>
      </PaperCard>

      {icsOpen && (
        <IcsUrlModal
          initialUrl={settings.icsUrl ?? ""}
          onSave={saveIcs}
          onClose={() => setIcsOpen(false)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 5. MusicCard
// ---------------------------------------------------------------------------

const MusicCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const track = snapshot.music.track;
  const cardBg = track?.pastelTint
    ? `rgb(${track.pastelTint[0]}, ${track.pastelTint[1]}, ${track.pastelTint[2]})`
    : "var(--rose)";
  const pct =
    track && track.duration > 0
      ? Math.max(0, Math.min(100, (track.elapsed / track.duration) * 100))
      : 0;

  return (
    <PaperCard
      title={track ? track.player === "appleMusic" ? "Music" : track.player === "spotify" ? "Spotify" : "Playing" : "Music"}
      bg={cardBg}
      width={CARD_WIDTHS.music}
      grain={grain}
    >
      {!track ? (
        <div className="nc-music-empty">
          <div className="nc-music-disc">♪</div>
          <div className="nc-empty-note">Nothing playing right now</div>
          <button
            className="nc-ink-btn primary"
            onClick={() => api.musicLaunchPlayer()}
          >
            Open Spotify
          </button>
        </div>
      ) : (
        <div className="nc-music-body">
          <div className="nc-music-top">
            {track.artworkDataUrl ? (
              <img
                src={track.artworkDataUrl}
                alt=""
                className="nc-album-art"
              />
            ) : (
              <div className="nc-album-art placeholder">♫</div>
            )}
            <div className="nc-track-meta">
              <div className="nc-track-title" title={track.title}>
                {track.title}
              </div>
              <div className="nc-track-artist" title={track.artist}>
                {track.artist || "Unknown artist"}
              </div>
              {track.album && (
                <div className="nc-track-album" title={track.album}>
                  {track.album}
                </div>
              )}
            </div>
          </div>

          <div className="nc-music-progress-wrap">
            <div className="nc-timer-progress-track">
              <div
                className="nc-timer-progress-fill"
                style={{ width: `${pct}%` }}
              />
            </div>
            <div className="nc-music-times">
              <span>{formatClock(track.elapsed)}</span>
              <span>{formatClock(track.duration)}</span>
            </div>
          </div>

          <div className="nc-music-controls">
            <button
              className="nc-ink-btn"
              onClick={() => api.musicPrevious()}
              title="Previous"
            >
              ⏮
            </button>
            <button
              className="nc-ink-btn primary"
              onClick={() => api.musicPlayPause()}
              title={track.playing ? "Pause" : "Play"}
            >
              {track.playing ? "⏸" : "▶"}
            </button>
            <button
              className="nc-ink-btn"
              onClick={() => api.musicNext()}
              title="Next"
            >
              ⏭
            </button>
          </div>
        </div>
      )}
    </PaperCard>
  );
};

// ---------------------------------------------------------------------------
// WorkspaceView Container
// ---------------------------------------------------------------------------

export const WorkspaceView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const { settings } = snapshot;
  const visible = settings.cards.filter((c) => c.enabled).map((c) => c.card);

  return (
    <div className="nc-workspace-row">
      {visible.map((cardId) => {
        switch (cardId) {
          case "tasks":
            return (
              <TasksCard
                key="tasks"
                snapshot={snapshot}
                grain={settings.paperGrain}
              />
            );
          case "timer":
            return (
              <TimerCard
                key="timer"
                snapshot={snapshot}
                grain={settings.paperGrain}
              />
            );
          case "notepad":
            return (
              <NotepadCard
                key="notepad"
                snapshot={snapshot}
                grain={settings.paperGrain}
              />
            );
          case "events":
            return (
              <EventsCard
                key="events"
                snapshot={snapshot}
                grain={settings.paperGrain}
              />
            );
          case "music":
            return (
              <MusicCard
                key="music"
                snapshot={snapshot}
                grain={settings.paperGrain}
              />
            );
        }
      })}
    </div>
  );
};
