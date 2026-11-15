import React, { useState } from "react";
import { api } from "../api";
import type { AppSnapshot } from "../types";

function formatDayColumnLabel(daySecs: number, index: number): {
  top: string;
  sub: string;
} {
  const d = new Date(daySecs * 1000);
  const sub = d.toLocaleDateString([], { month: "short", day: "numeric" });
  if (index === 0) return { top: "Today", sub };
  if (index === 1) return { top: "Tomorrow", sub };
  return {
    top: d.toLocaleDateString([], { weekday: "short" }),
    sub,
  };
}

export const WeekView: React.FC<{ snapshot: AppSnapshot }> = ({ snapshot }) => {
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [dragTaskId, setDragTaskId] = useState<string | null>(null);
  const [dragOverDay, setDragOverDay] = useState<number | null>(null);

  return (
    <div className="nc-week-grid">
      {snapshot.weekDays.map((col, idx) => {
        const label = formatDayColumnLabel(col.day, idx);
        const remaining = col.tasks.filter((t) => !t.isDone).length;
        const draft = drafts[col.day] ?? "";

        return (
          <div
            key={col.day}
            className={`nc-week-col ${idx === 0 ? "today" : ""} ${dragOverDay === col.day ? "drag-over" : ""}`}
            onDragOver={(e) => {
              e.preventDefault();
              setDragOverDay(col.day);
            }}
            onDragLeave={() => setDragOverDay(null)}
            onDrop={(e) => {
              e.preventDefault();
              setDragOverDay(null);
              const tid = dragTaskId || e.dataTransfer.getData("text/plain");
              if (tid) {
                api.moveTaskToDate(tid, col.day);
              }
              setDragTaskId(null);
              api.setDragActive(false);
            }}
          >
            <div className="nc-week-col-head">
              <div>
                <div className="nc-week-day-name">{label.top}</div>
                <div className="nc-week-day-date">{label.sub}</div>
              </div>
              {remaining > 0 && (
                <span className="nc-week-count">{remaining}</span>
              )}
            </div>

            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (!draft.trim()) return;
                api.addTask(draft.trim(), col.day);
                setDrafts((prev) => ({ ...prev, [col.day]: "" }));
              }}
            >
              <input
                className="nc-week-add-input"
                placeholder="+ Add…"
                value={draft}
                onChange={(e) =>
                  setDrafts((prev) => ({ ...prev, [col.day]: e.target.value }))
                }
              />
            </form>

            <div className="nc-week-list">
              {col.tasks.map((t) => (
                <div
                  key={t.id}
                  className={`nc-week-task ${t.isDone ? "done" : ""}`}
                  draggable
                  onDragStart={(e) => {
                    setDragTaskId(t.id);
                    api.setDragActive(true);
                    e.dataTransfer.setData("text/plain", t.id);
                  }}
                  onDragEnd={() => {
                    setDragTaskId(null);
                    setDragOverDay(null);
                    api.setDragActive(false);
                  }}
                >
                  <button
                    className={`nc-week-check ${t.isDone ? "checked" : ""}`}
                    onClick={() => api.toggleTask(t.id)}
                  >
                    {t.isDone ? "✓" : ""}
                  </button>
                  <span className="nc-week-task-title">{t.title}</span>
                  <button
                    className="nc-week-del"
                    onClick={() => api.deleteTask(t.id)}
                    title="Delete"
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
};
