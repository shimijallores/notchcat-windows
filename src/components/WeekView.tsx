import React, { useState } from "react";
import { CheckCircle2, Circle, Plus } from "lucide-react";
import { api } from "../api";
import type { AppSnapshot } from "../types";

const PALETTE = [
  "var(--mint)",
  "var(--lavender)",
  "var(--sand)",
  "var(--slate)",
  "var(--mint)",
  "var(--lavender)",
  "var(--sand)",
];

export const WeekView: React.FC<{ snapshot: AppSnapshot }> = ({ snapshot }) => {
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [dragTaskId, setDragTaskId] = useState<string | null>(null);
  const [dragOverDay, setDragOverDay] = useState<number | null>(null);
  const grain = snapshot.settings.paperGrain;

  return (
    <div className="nc-week-row">
      {snapshot.weekDays.map((col, idx) => {
        const d = new Date(col.day * 1000);
        const weekday = d
          .toLocaleDateString([], { weekday: "short" })
          .toUpperCase();
        const dayNum = d.getDate();
        const isToday = idx === 0;
        const doneCount = col.tasks.filter((t) => t.isDone).length;
        const draft = drafts[col.day] ?? "";
        const targeted = dragOverDay === col.day;

        const submit = () => {
          const title = draft.trim();
          if (!title) return;
          api.addTask(title, col.day);
          setDrafts((prev) => ({ ...prev, [col.day]: "" }));
        };

        return (
          <section
            key={col.day}
            className={`nc-week-card ${targeted ? "targeted" : ""}`}
            style={{ background: PALETTE[idx % PALETTE.length] }}
            onDragOver={(e) => {
              e.preventDefault();
              if (dragOverDay !== col.day) setDragOverDay(col.day);
            }}
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node)) {
                setDragOverDay((cur) => (cur === col.day ? null : cur));
              }
            }}
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
            {grain && <div className="nc-paper-grain" aria-hidden="true" />}

            <div className="nc-week-card-inner">
              <div className="nc-week-head">
                <div className="nc-week-weekday">{weekday}</div>
                <div className="nc-week-daynum-row">
                  <span className="nc-week-daynum">{dayNum}</span>
                  {isToday && <span className="nc-week-today-dot" />}
                  <span style={{ flex: 1 }} />
                  {col.tasks.length > 0 && (
                    <span className="nc-week-ratio">
                      {doneCount}/{col.tasks.length}
                    </span>
                  )}
                </div>
              </div>

              <div className="nc-week-list">
                {col.tasks.map((t) => (
                  <div
                    key={t.id}
                    className="nc-week-chip"
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
                      className="nc-week-chip-check"
                      onClick={() => api.toggleTask(t.id)}
                      aria-label={t.isDone ? "Mark not done" : "Mark done"}
                    >
                      {t.isDone ? (
                        <CheckCircle2
                          size={11}
                          strokeWidth={2.4}
                          style={{ opacity: 0.9 }}
                        />
                      ) : (
                        <Circle
                          size={11}
                          strokeWidth={2}
                          style={{ opacity: 0.5 }}
                        />
                      )}
                    </button>
                    <span
                      className={`nc-week-chip-title ${t.isDone ? "done" : ""}`}
                    >
                      {t.title}
                    </span>
                  </div>
                ))}
              </div>

              <div className="nc-week-add">
                <Plus
                  size={9}
                  strokeWidth={3}
                  style={{ opacity: 0.5, flexShrink: 0 }}
                />
                <input
                  value={draft}
                  placeholder="Add"
                  onChange={(e) =>
                    setDrafts((prev) => ({
                      ...prev,
                      [col.day]: e.target.value,
                    }))
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      submit();
                    }
                  }}
                />
              </div>
            </div>
          </section>
        );
      })}
    </div>
  );
};
