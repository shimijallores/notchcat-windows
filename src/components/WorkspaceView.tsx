import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  ArrowLeftCircle,
  ArrowRightCircle,
  Bell,
  BellOff,
  BellRing,
  Calendar,
  CalendarCheck,
  CheckCircle2,
  Circle,
  Clock,
  Copy,
  Ellipsis,
  GripVertical,
  Hourglass,
  Check,
  Music2,
  Music4,
  NotebookText,
  Pause,
  Pencil,
  Play,
  Plus,
  RotateCcw,
  Scan,
  SkipBack,
  SkipForward,
  Timer as TimerIcon,
  Trash2,
  ChevronRight,
} from "lucide-react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type { AppSnapshot, TaskItem } from "../types";
import { formatClock } from "../useNotchCat";
import { CustomReminderModal, FocusDurationModal, IcsUrlModal } from "./Modals";
import { PaperCard, SevenSegmentClock } from "./Visuals";

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

const timeShort = (ms: number) =>
  new Date(ms)
    .toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    .replace(/\s/g, " ");

/** "5:30 PM" for today, "Tue 9:00 AM" otherwise. */
function reminderLabel(secs: number): string {
  const d = new Date(secs * 1000);
  const t = timeShort(secs * 1000);
  const now = new Date();
  if (d.toDateString() === now.toDateString()) return t;
  const tomorrow = new Date(now.getTime() + 86400_000);
  if (d.toDateString() === tomorrow.toDateString()) return `Tomorrow ${t}`;
  return `${d.toLocaleDateString([], { weekday: "short" })} ${t}`;
}

function dayLabel(secs: number): string {
  const d = new Date(secs * 1000);
  const wk = d.toLocaleDateString("en-GB", { weekday: "short" });
  const mon = d.toLocaleDateString("en-GB", { month: "short" });
  return `${wk}, ${d.getDate()} ${mon}`;
}

function durationText(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

function useNow(intervalMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

function useElementWidth<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [width, setWidth] = useState(252);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);
  return [ref, width] as const;
}

const CardTitle: React.FC<{
  icon: React.ReactNode;
  title: string;
  right?: React.ReactNode;
  opacity?: number;
}> = ({ icon, title, right, opacity = 0.8 }) => (
  <div className="nc-card-title-row" style={{ opacity }}>
    {icon}
    <span className="nc-card-title">{title}</span>
    <span style={{ flex: 1 }} />
    {right}
  </div>
);

// ---------------------------------------------------------------------------
// Task context menu (portal so the card's overflow can't clip it)
// ---------------------------------------------------------------------------

interface MenuAction {
  label: string;
  icon: React.ReactNode;
  run: () => void;
  danger?: boolean;
}

const TaskMenu: React.FC<{
  anchor: DOMRect;
  groups: MenuAction[][];
  remind?: { label: string; run: () => void }[];
  remindExtra?: { label: string; run: () => void; danger?: boolean }[];
  onClose: () => void;
}> = ({ anchor, groups, remind, remindExtra, onClose }) => {
  const ref = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState({ left: anchor.right - 190, top: anchor.bottom + 4 });
  const [remindOpen, setRemindOpen] = useState(false);

  useLayoutEffect(() => {
    const el = ref.current;
    const host = document.querySelector(".nc-tray") as HTMLElement | null;
    if (!el || !host) return;
    const hr = host.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let left = anchor.right - w;
    let top = anchor.bottom + 4;
    left = Math.max(hr.left + 8, Math.min(left, hr.right - w - 8));
    if (top + h > hr.bottom - 8) top = Math.max(hr.top + 36, anchor.top - h - 4);
    setPos({ left, top });
  }, [anchor, remindOpen]);

  useEffect(() => {
    const down = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("keydown", key, true);
    };
  }, [onClose]);

  const host = document.querySelector(".nc-tray") ?? document.body;
  const fire = (fn: () => void) => () => {
    onClose();
    fn();
  };

  return createPortal(
    <div
      ref={ref}
      className="nc-menu"
      style={{ left: pos.left, top: pos.top }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {groups.map((group, gi) => (
        <React.Fragment key={gi}>
          {gi > 0 && <div className="nc-menu-sep" />}
          {group.map((a) => (
            <button
              key={a.label}
              className={a.danger ? "danger" : ""}
              onClick={fire(a.run)}
            >
              {a.icon}
              <span>{a.label}</span>
            </button>
          ))}
          {gi === 0 && remind && (
            <>
              <button onClick={() => setRemindOpen((v) => !v)}>
                <Bell size={13} />
                <span>Remind Me</span>
                <ChevronRight
                  size={12}
                  style={{
                    marginLeft: "auto",
                    transform: remindOpen ? "rotate(90deg)" : undefined,
                  }}
                />
              </button>
              {remindOpen && (
                <div className="nc-menu-sub">
                  {remind.map((r) => (
                    <button key={r.label} onClick={fire(r.run)}>
                      <span>{r.label}</span>
                    </button>
                  ))}
                  {remindExtra && remindExtra.length > 0 && <div className="nc-menu-sep" />}
                  {remindExtra?.map((r) => (
                    <button key={r.label} className={r.danger ? "danger" : ""} onClick={fire(r.run)}>
                      <span>{r.label}</span>
                    </button>
                  ))}
                </div>
              )}
            </>
          )}
        </React.Fragment>
      ))}
    </div>,
    host,
  );
};

// ---------------------------------------------------------------------------
// 1. Tasks
// ---------------------------------------------------------------------------

const TasksCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const { timer, settings } = snapshot;
  const [day, setDay] = useState<"today" | "tomorrow">("today");
  const [draft, setDraft] = useState("");
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameText, setRenameText] = useState("");
  const [menu, setMenu] = useState<{ task: TaskItem; rect: DOMRect } | null>(null);
  const [reminderTask, setReminderTask] = useState<TaskItem | null>(null);
  const [limitTask, setLimitTask] = useState<TaskItem | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const now = useNow(30_000);

  const dayTs =
    day === "today"
      ? snapshot.todayStartSecs
      : snapshot.weekDays[1]?.day ?? snapshot.todayStartSecs + 86400;
  const tasks = day === "today" ? snapshot.todayTasks : snapshot.tomorrowTasks;
  const doneCount = tasks.filter((t) => t.isDone).length;

  const submit = () => {
    const title = draft.trim();
    if (!title) return;
    api.addTask(title, dayTs);
    setDraft("");
  };

  const commitRename = (id: string) => {
    const v = renameText.trim();
    if (v) api.renameTask(id, v);
    setRenamingId(null);
  };

  const drop = (targetId: string) => {
    if (!dragId || dragId === targetId) return;
    const ids = tasks.map((t) => t.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return;
    ids.splice(from, 1);
    ids.splice(to, 0, dragId);
    api.reorderTasks(dayTs, ids);
    setDragId(null);
    api.setDragActive(false);
  };

  // Seconds of focus still needed across unfinished tasks that have a time limit.
  const estimateSeconds = tasks
    .filter((t) => !t.isDone)
    .reduce((total, t) => {
      if (!t.timeLimitMinutes) return total;
      const inSession =
        timer.linkedTaskId === t.id &&
        (timer.state === "running" || timer.state === "paused");
      return (
        total +
        (inSession
          ? Math.min(timer.remaining, t.timeLimitMinutes * 60)
          : t.timeLimitMinutes * 60)
      );
    }, 0);
  let estimate: string | null = null;
  if (estimateSeconds > 0) {
    const minutes = Math.ceil(estimateSeconds / 60);
    const dur = durationText(minutes);
    if (day !== "today") {
      estimate = `${dur} planned`;
    } else {
      const finish = new Date(now + minutes * 60_000);
      let clock = timeShort(finish.getTime()).replace(/\s/g, "").toLowerCase();
      if (finish.toDateString() !== new Date(now).toDateString())
        clock = `tomorrow ${clock}`;
      estimate = `${clock} (${dur})`;
    }
  }

  const focus = (t: TaskItem) =>
    api.timerStart(t.timeLimitMinutes ?? settings.defaultFocusMinutes, t.id);

  const menuGroups = (t: TaskItem): MenuAction[][] => [
    [
      { label: "Focus", icon: <Scan size={13} />, run: () => focus(t) },
      {
        label: t.isDone ? "Mark as Not Done" : "Mark as Done",
        icon: <CheckCircle2 size={13} />,
        run: () => api.toggleTask(t.id),
      },
      {
        label: "Rename…",
        icon: <Pencil size={13} />,
        run: () => {
          setRenameText(t.title);
          setRenamingId(t.id);
        },
      },
      {
        label: "Set Time Limit…",
        icon: <Clock size={13} />,
        run: () => setLimitTask(t),
      },
    ],
    [
      {
        label: day === "today" ? "Move to Tomorrow" : "Move to Today",
        icon: day === "today" ? <ArrowRightCircle size={13} /> : <ArrowLeftCircle size={13} />,
        run: () =>
          api.moveTaskToDate(
            t.id,
            day === "today"
              ? snapshot.weekDays[1]?.day ?? snapshot.todayStartSecs + 86400
              : snapshot.todayStartSecs,
          ),
      },
      { label: "Duplicate", icon: <Copy size={13} />, run: () => api.duplicateTask(t.id) },
    ],
    [
      {
        label: "Delete",
        icon: <Trash2 size={13} />,
        run: () => api.deleteTask(t.id),
        danger: true,
      },
    ],
  ];

  const badges = (t: TaskItem) => {
    const linked = timer.linkedTaskId === t.id && timer.state !== "idle";
    if (!t.timeLimitMinutes && !t.remindAt && !linked) return null;
    return (
      <div className="nc-badges">
        {t.timeLimitMinutes ? (
          <span className="nc-badge">
            <Clock size={9} strokeWidth={3} />
            {t.timeLimitMinutes}m
          </span>
        ) : null}
        {t.remindAt ? (
          <span className="nc-badge" style={{ opacity: t.reminded ? 0.55 : 1 }}>
            {t.reminded ? <BellOff size={9} strokeWidth={3} /> : <Bell size={9} strokeWidth={3} />}
            {reminderLabel(t.remindAt)}
          </span>
        ) : null}
        {linked && (
          <span className="nc-badge active">
            <i className={`nc-dot ${timer.state}`} />
            {timer.state === "running" ? "Active" : timer.state === "paused" ? "Paused" : "Time's up"}
          </span>
        )}
      </div>
    );
  };

  return (
    <>
      <PaperCard bg="var(--mint)" grain={grain} weight={1.5}>
        <div className="nc-tasks">
          <div className="nc-tasks-head">
            {(["today", "tomorrow"] as const).map((d) => (
              <button
                key={d}
                className={`nc-day-tab ${day === d ? "active" : ""}`}
                onClick={() => setDay(d)}
              >
                {d === "today" ? "Today" : "Tomorrow"}
              </button>
            ))}
            <span style={{ flex: 1 }} />
            <span className="nc-counter">
              {doneCount}/{tasks.length}
            </span>
          </div>

          <div className="nc-add-field">
            <Plus size={12} strokeWidth={3} style={{ opacity: 0.55 }} />
            <input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
              placeholder="What needs doing?"
            />
          </div>

          {tasks.length === 0 ? (
            <div className="nc-empty">
              <img src={mascotUrl} alt="" width={44} height={44} style={{ opacity: 0.85 }} />
              <span>
                {day === "today" ? "Nothing planned yet." : "Nothing queued for tomorrow."}
              </span>
            </div>
          ) : (
            <div className="nc-scroll-list">
              {tasks.map((t) => (
                <div
                  key={t.id}
                  className={`nc-task-row ${dragId === t.id ? "dragging" : ""}`}
                  draggable={renamingId !== t.id}
                  onDragStart={(e) => {
                    setDragId(t.id);
                    api.setDragActive(true);
                    e.dataTransfer.setData("text/plain", t.id);
                  }}
                  onDragEnd={() => {
                    setDragId(null);
                    api.setDragActive(false);
                  }}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={() => drop(t.id)}
                >
                  <GripVertical size={12} strokeWidth={2.5} className="nc-grip" />
                  <button
                    className="nc-check"
                    onClick={() => api.toggleTask(t.id)}
                    aria-label={t.isDone ? "Mark not done" : "Mark done"}
                  >
                    {t.isDone ? (
                      <CheckCircle2 size={18} strokeWidth={2.2} />
                    ) : (
                      <Circle size={18} strokeWidth={1.8} style={{ opacity: 0.45 }} />
                    )}
                  </button>
                  <div className="nc-task-text">
                    {renamingId === t.id ? (
                      <input
                        className="nc-rename"
                        value={renameText}
                        autoFocus
                        onChange={(e) => setRenameText(e.target.value)}
                        onBlur={() => commitRename(t.id)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") commitRename(t.id);
                          else if (e.key === "Escape") {
                            e.stopPropagation();
                            setRenamingId(null);
                          }
                        }}
                      />
                    ) : (
                      <span
                        className={`nc-task-title ${t.isDone ? "done" : ""}`}
                        onDoubleClick={() => {
                          setRenameText(t.title);
                          setRenamingId(t.id);
                        }}
                      >
                        {t.title}
                      </span>
                    )}
                    {badges(t)}
                  </div>
                  <button
                    className="nc-more"
                    aria-label="More"
                    onClick={(e) =>
                      setMenu({
                        task: t,
                        rect: (e.currentTarget as HTMLElement).getBoundingClientRect(),
                      })
                    }
                  >
                    <Ellipsis size={15} strokeWidth={3} />
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="nc-tasks-foot">
            {estimate && (
              <span className="nc-foot-estimate" title="Estimated time to finish every unfinished task that has a time limit">
                <Hourglass size={10.5} />
                {estimate}
              </span>
            )}
            <span style={{ flex: 1 }} />
            <span>{dayLabel(dayTs)}</span>
          </div>
        </div>
      </PaperCard>

      {menu && (
        <TaskMenu
          anchor={menu.rect}
          groups={menuGroups(menu.task)}
          remind={
            settings.remindersEnabled
              ? [
                  ...snapshot.reminderPresets.map((p) => ({
                    label: p.title,
                    run: () => {
                      api.setTaskReminder(menu.task.id, p.timestamp);
                    },
                  })),
                  { label: "Custom…", run: () => setReminderTask(menu.task) },
                ]
              : undefined
          }
          remindExtra={
            menu.task.remindAt
              ? [
                  {
                    label: "Clear Reminder",
                    run: () => api.setTaskReminder(menu.task.id, null),
                  },
                ]
              : []
          }
          onClose={() => setMenu(null)}
        />
      )}

      {reminderTask && (
        <CustomReminderModal
          taskTitle={reminderTask.title}
          initialTimestamp={reminderTask.remindAt}
          onSave={(ts) => api.setTaskReminder(reminderTask.id, ts)}
          onClear={() => api.setTaskReminder(reminderTask.id, null)}
          onClose={() => setReminderTask(null)}
        />
      )}

      {limitTask && (
        <FocusDurationModal
          initialMinutes={limitTask.timeLimitMinutes ?? settings.defaultFocusMinutes}
          allowsClear={limitTask.timeLimitMinutes !== null}
          onSave={(m) => api.setTaskTimeLimit(limitTask.id, m)}
          onClear={() => api.setTaskTimeLimit(limitTask.id, null)}
          onClose={() => setLimitTask(null)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 2. Timer
// ---------------------------------------------------------------------------

const TimerCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const { timer, todayTasks, tomorrowTasks } = snapshot;
  const [durationOpen, setDurationOpen] = useState(false);
  const [ref, width] = useElementWidth<HTMLDivElement>();
  const compact = width < 215;

  const linked = timer.linkedTaskId
    ? [...todayTasks, ...tomorrowTasks].find((t) => t.id === timer.linkedTaskId)
    : null;
  const active = timer.state === "running" || timer.state === "paused";
  const status =
    timer.state === "idle"
      ? "Ready"
      : timer.state === "running"
        ? "Remaining"
        : timer.state === "paused"
          ? "Paused"
          : "Time's up";

  return (
    <>
      <PaperCard bg="var(--lavender)" grain={grain}>
        <div className="nc-timer" ref={ref}>
          <CardTitle
            icon={<TimerIcon size={12} strokeWidth={2.5} />}
            title={linked?.title ?? "Focus"}
            opacity={0.7}
          />

          <div className="nc-timer-mid">
            {timer.state === "finished" ? (
              <div className="nc-timesup">
                <BellRing size={22} style={{ opacity: 0.8 }} />
                <b>Time's up</b>
                <span>Nice work. Take a breath.</span>
              </div>
            ) : (
              <>
                <SevenSegmentClock
                  text={formatClock(timer.remaining)}
                  dim={timer.state === "paused"}
                  maxHeight={66}
                />
                <div className="nc-timer-status">{status}</div>
                <div className="nc-progress">
                  <div style={{ width: `${Math.max(1, timer.progress * 100)}%` }} />
                </div>
              </>
            )}
          </div>

          <div className="nc-timer-controls">
            {timer.state === "finished" ? (
              <>
                <button className="nc-btn-primary" onClick={() => api.timerAddTime(300)}>
                  + Add 5 min
                </button>
                <button className="nc-btn-secondary" onClick={() => api.timerReset()}>
                  <Check size={12} strokeWidth={3} />
                  Done
                </button>
              </>
            ) : (
              <>
                <button className="nc-btn-primary" onClick={() => api.timerToggle()}>
                  {timer.state === "running" ? (
                    <Pause size={12} fill="currentColor" />
                  ) : (
                    <Play size={12} fill="currentColor" />
                  )}
                  {timer.state === "running"
                    ? "Pause"
                    : timer.state === "paused"
                      ? "Resume"
                      : "Start"}
                </button>
                {active ? (
                  <button
                    className="nc-btn-secondary icon"
                    title="Reset"
                    onClick={() => api.timerReset()}
                  >
                    <RotateCcw size={13} strokeWidth={2.5} />
                  </button>
                ) : (
                  <button
                    className={`nc-btn-secondary ${compact ? "icon" : ""}`}
                    title="Set time"
                    onClick={() => setDurationOpen(true)}
                  >
                    <Clock size={13} strokeWidth={2.5} />
                    {!compact && "Set time"}
                  </button>
                )}
              </>
            )}
          </div>
        </div>
      </PaperCard>

      {durationOpen && (
        <FocusDurationModal
          initialMinutes={Math.max(1, Math.round(timer.sessionTotal / 60))}
          onSave={(m) => {
            api.timerSetDuration(m);
          }}
          onClose={() => setDurationOpen(false)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 3. Notepad
// ---------------------------------------------------------------------------

const NotepadCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [text, setText] = useState(snapshot.noteText);
  const [flash, setFlash] = useState<string | null>(null);
  const areaRef = useRef<HTMLTextAreaElement | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    setText(snapshot.noteText);
  }, [snapshot.noteText]);

  const change = (v: string) => {
    setText(v);
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => api.noteSetText(v), 220);
  };

  const convert = async () => {
    const cursor = areaRef.current?.selectionStart ?? 0;
    if (timer.current) {
      window.clearTimeout(timer.current);
      timer.current = null;
      await api.noteSetText(text);
    }
    const res = await api.noteConvertLine(cursor);
    if (res.converted) {
      setText(res.nextText);
      setFlash("Added to Today");
      window.setTimeout(() => setFlash(null), 1400);
      requestAnimationFrame(() => {
        const el = areaRef.current;
        if (el) el.selectionStart = el.selectionEnd = res.cursorUtf16;
      });
    }
  };

  const words = snapshot.noteWordCount;

  return (
    <PaperCard bg="var(--sand)" grain={grain}>
      <div className="nc-notepad">
        <CardTitle icon={<NotebookText size={12} strokeWidth={2.5} />} title="Notepad" />
        <textarea
          ref={areaRef}
          className="nc-note-area"
          value={text}
          spellCheck={false}
          onChange={(e) => change(e.target.value)}
          onBlur={() => text !== snapshot.noteText && api.noteSetText(text)}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
              e.preventDefault();
              convert();
            }
          }}
          placeholder="Write something down…"
        />
        <div className="nc-note-foot" style={{ opacity: flash ? 0.9 : 0.5 }}>
          <span>
            {words} {words === 1 ? "word" : "words"}
          </span>
          {snapshot.noteHasUndo && !flash && (
            <button className="nc-linklike" onClick={() => api.noteUndo()}>
              Undo
            </button>
          )}
          <span style={{ flex: 1 }} />
          <span
            style={{
              color: flash ? "rgb(31,102,51)" : undefined,
              fontWeight: flash ? 600 : 500,
            }}
          >
            {flash ?? "Ctrl+↵ makes a task"}
          </span>
        </div>
      </div>
    </PaperCard>
  );
};

// ---------------------------------------------------------------------------
// 4. Events
// ---------------------------------------------------------------------------

interface EventItem {
  id: string;
  title: string;
  start: number;
  end: number | null;
  isReminder: boolean;
  isAllDay: boolean;
}

const EventsCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const [icsOpen, setIcsOpen] = useState(false);
  const { calendar, todayReminders, settings } = snapshot;
  const now = useNow(30_000);

  const items: EventItem[] = calendar.events.map((e) => ({
    id: `e-${e.id}`,
    title: e.title,
    start: e.startMs,
    end: e.endMs,
    isReminder: false,
    isAllDay: e.isAllDay,
  }));
  if (settings.showRemindersInEvents) {
    todayReminders.forEach((t) => {
      if (t.remindAt)
        items.push({
          id: `r-${t.id}`,
          title: t.title,
          start: t.remindAt * 1000,
          end: null,
          isReminder: true,
          isAllDay: false,
        });
    });
  }
  items.sort(
    (a, b) =>
      Number(!a.isAllDay) - Number(!b.isAllDay) || a.start - b.start,
  );

  const authorized = calendar.access.kind === "authorized";
  const timeText = (i: EventItem) => {
    if (i.isAllDay) return "All day";
    return i.end ? `${timeShort(i.start)} – ${timeShort(i.end)}` : timeShort(i.start);
  };

  return (
    <>
      <PaperCard bg="var(--slate)" grain={grain}>
        <div className="nc-events">
          <CardTitle icon={<Calendar size={12} strokeWidth={2.5} />} title="Today" opacity={0.85} />

          {items.length === 0 ? (
            <div className="nc-empty light">
              {authorized ? (
                <CalendarCheck size={26} strokeWidth={1.4} style={{ opacity: 0.45 }} />
              ) : (
                <Calendar size={26} strokeWidth={1.4} style={{ opacity: 0.45 }} />
              )}
              <span>{authorized ? "Nothing scheduled today." : "No reminders today."}</span>
            </div>
          ) : (
            <div className="nc-scroll-list events">
              {items.map((i) => {
                const happeningNow =
                  !i.isReminder &&
                  !i.isAllDay &&
                  i.start <= now &&
                  now < (i.end ?? i.start);
                const upcoming =
                  i.isReminder || i.isAllDay || (i.end ?? i.start) >= now;
                return (
                  <div
                    key={i.id}
                    className="nc-event"
                    style={{ opacity: upcoming ? 1 : 0.5 }}
                  >
                    <span
                      className="nc-event-bar"
                      style={{
                        background: i.isReminder
                          ? "var(--amber)"
                          : "rgba(27,29,28,0.55)",
                      }}
                    />
                    <div className="nc-event-info">
                      <div className="nc-event-title">{i.title}</div>
                      <div className="nc-event-time">
                        {i.isReminder && <Bell size={9} fill="currentColor" />}
                        {timeText(i)}
                      </div>
                      {happeningNow && <span className="nc-now-badge">Happening now</span>}
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          <div style={{ flex: 1 }} />
          {calendar.access.kind === "failed" && (
            <div className="nc-footnote">Feed error: {calendar.access.message}</div>
          )}
          {calendar.access.kind === "notDetermined" ? (
            <button className="nc-btn-ink-wide" onClick={() => setIcsOpen(true)}>
              Connect Calendar
            </button>
          ) : (
            <button className="nc-linklike foot" onClick={() => setIcsOpen(true)}>
              Calendar feed
            </button>
          )}
        </div>
      </PaperCard>

      {icsOpen && (
        <IcsUrlModal
          initialUrl={settings.icsUrl ?? ""}
          onSave={(url) => api.settingsUpdate({ ...settings, icsUrl: url || null })}
          onClose={() => setIcsOpen(false)}
        />
      )}
    </>
  );
};

// ---------------------------------------------------------------------------
// 5. Music
// ---------------------------------------------------------------------------

const PLAYER_NAME: Record<string, string> = {
  spotify: "Spotify",
  appleMusic: "Apple Music",
  systemMedia: "Windows Media",
};

const mmss = (s: number) => {
  const v = Math.max(0, Math.round(s));
  return `${Math.floor(v / 60)}:${String(v % 60).padStart(2, "0")}`;
};

const MusicCard: React.FC<{ snapshot: AppSnapshot; grain: boolean }> = ({
  snapshot,
  grain,
}) => {
  const track = snapshot.music.track;
  const now = useNow(1000);
  const bg = track?.pastelTint
    ? `rgb(${track.pastelTint[0]}, ${track.pastelTint[1]}, ${track.pastelTint[2]})`
    : "var(--rose)";

  let position = 0;
  if (track) {
    position = track.elapsed + (track.playing ? (now - track.updatedAtMs) / 1000 : 0);
    if (track.duration > 0) position = Math.min(position, track.duration);
  }
  const fraction = track && track.duration > 0 ? position / track.duration : 0;

  return (
    <PaperCard bg={bg} grain={grain}>
      <div className="nc-music">
        <CardTitle
          icon={<Music2 size={12} strokeWidth={2.5} />}
          title="Now playing"
          opacity={0.85}
          right={
            track && (
              <span className="nc-music-source">
                {PLAYER_NAME[track.player] ?? "Media"}
              </span>
            )
          }
        />

        {track ? (
          <div className="nc-player">
            <div className="nc-art">
              {track.artworkDataUrl ? (
                <img src={track.artworkDataUrl} alt="" />
              ) : (
                <Music2 size={28} style={{ opacity: 0.35 }} />
              )}
            </div>
            <div className="nc-track">
              <div className="nc-track-title" title={track.title}>
                {track.title}
              </div>
              <div className="nc-track-artist" title={track.artist}>
                {track.artist || track.album}
              </div>
            </div>
            <div className="nc-music-progress">
              <span style={{ width: 34, textAlign: "right" }}>{mmss(position)}</span>
              <div className="nc-music-bar">
                <div style={{ width: `${Math.max(1, fraction * 100)}%` }} />
              </div>
              <span style={{ width: 34 }}>{mmss(track.duration)}</span>
            </div>
            <div className="nc-music-controls">
              <button className="nc-transport" onClick={() => api.musicPrevious()} title="Previous">
                <SkipBack size={14} fill="currentColor" />
              </button>
              <button
                className="nc-play"
                onClick={() => api.musicPlayPause()}
                title={track.playing ? "Pause" : "Play"}
              >
                {track.playing ? (
                  <Pause size={16} fill="currentColor" />
                ) : (
                  <Play size={16} fill="currentColor" style={{ marginLeft: 2 }} />
                )}
              </button>
              <button className="nc-transport" onClick={() => api.musicNext()} title="Next">
                <SkipForward size={14} fill="currentColor" />
              </button>
            </div>
          </div>
        ) : (
          <div className="nc-empty light" style={{ gap: 10 }}>
            <Music4 size={26} strokeWidth={1.4} style={{ opacity: 0.4 }} />
            <span style={{ fontWeight: 600, opacity: 0.65 }}>Nothing playing</span>
            <button className="nc-btn-ink-wide" onClick={() => api.musicLaunchPlayer()}>
              Open Spotify
            </button>
          </div>
        )}
      </div>
    </PaperCard>
  );
};

// ---------------------------------------------------------------------------
// WorkspaceView
// ---------------------------------------------------------------------------

export const WorkspaceView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const { settings } = snapshot;
  const visible = settings.cards.filter((c) => c.enabled).map((c) => c.card);
  const grain = settings.paperGrain;

  return (
    <div className="nc-workspace-row">
      {visible.map((id) => {
        switch (id) {
          case "tasks":
            return <TasksCard key={id} snapshot={snapshot} grain={grain} />;
          case "timer":
            return <TimerCard key={id} snapshot={snapshot} grain={grain} />;
          case "notepad":
            return <NotepadCard key={id} snapshot={snapshot} grain={grain} />;
          case "events":
            return <EventsCard key={id} snapshot={snapshot} grain={grain} />;
          case "music":
            return <MusicCard key={id} snapshot={snapshot} grain={grain} />;
          default:
            return null;
        }
      })}
    </div>
  );
};
