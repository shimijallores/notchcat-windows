import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  Calendar,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Ellipsis,
  RotateCcw,
  Search,
  Timer,
  X,
} from "lucide-react";
import { api } from "../api";
import type { AppSnapshot, HistoryPage, InsightsDay } from "../types";
import { formatDurationShort } from "../useNotchCat";

type Metric = "tasks" | "focus" | "history";

function formatAxisLabel(v: number, metric: Metric): string {
  if (metric === "tasks") return String(Math.round(v));
  if (v === 0) return "0";
  const m = Math.round(v);
  return m % 60 === 0 ? `${m / 60}h` : `${m}m`;
}

function formatRange(days: InsightsDay[]): string {
  if (days.length === 0) return "";
  const fmt = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString("en-GB", {
      day: "numeric",
      month: "short",
    });
  return `${fmt(days[0].date)} – ${fmt(days[days.length - 1].date)}`;
}

const HistoryRestoreMenu: React.FC<{
  anchor: DOMRect;
  onRestore: () => void;
  onClose: () => void;
}> = ({ anchor, onRestore, onClose }) => {
  const ref = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState({ left: anchor.right - 150, top: anchor.bottom + 4 });

  useLayoutEffect(() => {
    const el = ref.current;
    const host = document.querySelector(".nc-tray") as HTMLElement | null;
    if (!el || !host) return;
    const hr = host.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let left = Math.max(hr.left + 8, Math.min(anchor.right - w, hr.right - w - 8));
    let top = anchor.bottom + 4;
    if (top + h > hr.bottom - 8) top = Math.max(hr.top + 36, anchor.top - h - 4);
    setPos({ left, top });
  }, [anchor]);

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
  return createPortal(
    <div
      ref={ref}
      className="nc-menu"
      style={{ left: pos.left, top: pos.top, minWidth: 165 }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <button
        onClick={() => {
          onClose();
          onRestore();
        }}
      >
        <RotateCcw size={13} />
        <span>Restore to Today</span>
      </button>
    </div>,
    host,
  );
};

export const InsightsView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const [metric, setMetric] = useState<Metric>("tasks");
  const [selectedDate, setSelectedDate] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [pageSize, setPageSize] = useState(60);
  const [history, setHistory] = useState<HistoryPage | null>(null);
  const [menuTarget, setMenuTarget] = useState<{ id: string; rect: DOMRect } | null>(
    null,
  );

  const { insights, ui, settings } = snapshot;
  const grain = settings.paperGrain;

  const fetchHistory = async (q: string, size: number) => {
    const res = await api.getHistory(q, size);
    setHistory(res);
  };

  useEffect(() => {
    if (metric === "history") {
      fetchHistory(search, pageSize);
    }
  }, [metric, search, pageSize, snapshot.todayTasks, snapshot.tomorrowTasks, insights.completed]);

  const values = (d: InsightsDay): { dark: number; total: number } => {
    if (metric === "tasks") {
      return { dark: d.completed, total: Math.max(d.planned, d.completed) };
    }
    const mins = d.focusSeconds / 60;
    return { dark: mins, total: mins };
  };

  const peak = Math.max(0, ...insights.days.map((d) => values(d).total));
  const axisMax =
    metric === "tasks"
      ? peak <= 4
        ? 4
        : Math.ceil(peak / 4) * 4
      : peak <= 60
        ? 60
        : Math.ceil(peak / 60) * 60;

  const selectedDay =
    selectedDate !== null
      ? insights.days.find((d) => d.date === selectedDate) ?? null
      : null;

  const detailText = (() => {
    if (!selectedDay) return "Select a day";
    const d = new Date(selectedDay.date * 1000);
    const dayStr = `${d.toLocaleDateString("en-GB", { weekday: "long" })}, ${d.getDate()} ${d.toLocaleDateString("en-GB", { month: "short" })}`;
    if (metric === "tasks") {
      return `${dayStr}: ${selectedDay.completed} done of ${selectedDay.planned} planned`;
    }
    return `${dayStr}: ${formatDurationShort(selectedDay.focusSeconds)} focused`;
  })();

  const todaySec = snapshot.todayStartSecs;

  return (
    <div className="nc-insights-row">
      {/* Left summary card */}
      <section className="nc-paper-card nc-insights-summary" style={{ background: "var(--mint)" }}>
        {grain && <div className="nc-paper-grain" aria-hidden="true" />}
        <div className="nc-insights-summary-inner">
          <div className="nc-card-title-row" style={{ opacity: 0.75 }}>
            <Calendar size={12} strokeWidth={2.5} />
            <span className="nc-card-title">This week</span>
          </div>

          <h2 className="nc-insights-headline">{insights.headline}</h2>
          <p className="nc-insights-sentence">{insights.sentence}</p>

          <span style={{ flex: 1 }} />

          <div className="nc-insights-big-row">
            <span className="nc-insights-big-num">{insights.completed}</span>
            <div className="nc-insights-big-labels">
              <b>Tasks completed</b>
              <span>of {insights.planned} planned</span>
            </div>
          </div>

          <div className="nc-stat-chips">
            <div className="nc-stat-chip">
              <span className="nc-stat-chip-title">Focus time</span>
              <span className="nc-stat-chip-val">
                {formatDurationShort(insights.focusSeconds)}
              </span>
            </div>
            <div className="nc-stat-chip">
              <span className="nc-stat-chip-title">Active days</span>
              <span className="nc-stat-chip-val">{insights.activeDays}/7</span>
            </div>
          </div>
        </div>
      </section>

      {/* Right chart / history card */}
      <section className="nc-paper-card nc-insights-right" style={{ background: "var(--lavender)" }}>
        {grain && <div className="nc-paper-grain" aria-hidden="true" />}
        <div className="nc-insights-right-inner">
          <div className="nc-insights-toolbar">
            <div className="nc-seg-ink">
              {(["tasks", "focus", "history"] as Metric[]).map((m) => (
                <button
                  key={m}
                  className={metric === m ? "active" : ""}
                  onClick={() => setMetric(m)}
                >
                  {m === "tasks" ? "Tasks" : m === "focus" ? "Focus" : "History"}
                </button>
              ))}
            </div>

            <span style={{ flex: 1 }} />

            {metric === "history" ? (
              <div className="nc-hist-search">
                <Search size={11} strokeWidth={2.5} style={{ opacity: 0.5, flexShrink: 0 }} />
                <input
                  value={search}
                  placeholder="Search finished tasks"
                  onChange={(e) => {
                    setSearch(e.target.value);
                    setPageSize(60);
                  }}
                />
                {search && (
                  <button
                    className="nc-hist-clear"
                    onClick={() => {
                      setSearch("");
                      setPageSize(60);
                    }}
                    aria-label="Clear search"
                  >
                    <X size={10} strokeWidth={3} />
                  </button>
                )}
              </div>
            ) : (
              <div className="nc-week-nav">
                <button
                  className="nc-week-btn"
                  onClick={() => {
                    api.setInsightsWeekOffset(ui.insightsWeekOffset - 1);
                    setSelectedDate(null);
                  }}
                  title="Previous week"
                >
                  <ChevronLeft size={11} strokeWidth={3} />
                </button>
                <span className="nc-week-range">{formatRange(insights.days)}</span>
                <button
                  className="nc-week-btn"
                  disabled={ui.insightsWeekOffset >= 0}
                  onClick={() => {
                    api.setInsightsWeekOffset(ui.insightsWeekOffset + 1);
                    setSelectedDate(null);
                  }}
                  title="Next week"
                >
                  <ChevronRight size={11} strokeWidth={3} />
                </button>
              </div>
            )}
          </div>

          {metric === "history" ? (
            <div className="nc-history-wrap">
              {!history || history.sections.length === 0 ? (
                <div className="nc-empty">
                  {search.trim() ? (
                    <Search size={24} style={{ opacity: 0.35 }} />
                  ) : (
                    <CheckCircle2 size={24} style={{ opacity: 0.35 }} />
                  )}
                  <span>
                    {search.trim()
                      ? "No finished tasks match."
                      : "Finished tasks will show up here."}
                  </span>
                </div>
              ) : (
                <>
                  <div className="nc-history-scroll">
                    {history.sections.map((sec) => (
                      <div key={sec.day} className="nc-hist-section">
                        <div className="nc-hist-sec-head">
                          <b>{sec.title}</b>
                          <span>{sec.items.length} done</span>
                          {sec.focusSeconds > 0 && (
                            <span>· {formatDurationShort(sec.focusSeconds)} focused</span>
                          )}
                        </div>
                        {sec.items.map((item) => {
                          const completedStr = new Date(
                            item.completedAt * 1000,
                          ).toLocaleTimeString([], {
                            hour: "numeric",
                            minute: "2-digit",
                          });
                          return (
                            <div key={item.id} className="nc-hist-row">
                              <CheckCircle2
                                size={14}
                                strokeWidth={2.2}
                                style={{ opacity: 0.8, flexShrink: 0 }}
                              />
                              <span className="nc-hist-title">{item.title}</span>
                              {item.focusSeconds > 0 && (
                                <span className="nc-badge">
                                  <Timer size={9} strokeWidth={3} />
                                  {formatDurationShort(item.focusSeconds)}
                                </span>
                              )}
                              <span className="nc-hist-time">{completedStr}</span>
                              <button
                                className="nc-hist-more"
                                aria-label="Task actions"
                                onClick={(e) =>
                                  setMenuTarget({
                                    id: item.id,
                                    rect: (
                                      e.currentTarget as HTMLElement
                                    ).getBoundingClientRect(),
                                  })
                                }
                              >
                                <Ellipsis size={13} strokeWidth={3} />
                              </button>
                            </div>
                          );
                        })}
                      </div>
                    ))}
                    {history.canLoadMore && (
                      <button
                        className="nc-show-older"
                        onClick={() => setPageSize((s) => s + 60)}
                      >
                        Show older
                      </button>
                    )}
                  </div>
                  <div className="nc-history-foot">
                    {history.total} finished {history.total === 1 ? "task" : "tasks"}
                  </div>
                </>
              )}
            </div>
          ) : (
            <div className="nc-chart-area-wrap">
              <div className="nc-chart-grid">
                <div className="nc-y-axis">
                  {[0, 1, 2, 3, 4].map((i) => (
                    <span
                      key={i}
                      className="nc-y-label"
                      style={{ top: `${(i / 4) * 100}%` }}
                    >
                      {formatAxisLabel((axisMax * (4 - i)) / 4, metric)}
                    </span>
                  ))}
                </div>

                <div className="nc-plot-col">
                  <div className="nc-plot-bars">
                    {[0, 1, 2, 3, 4].map((i) => (
                      <div
                        key={i}
                        className={`nc-gridline ${i === 4 ? "base" : ""}`}
                        style={{ top: `${(i / 4) * 100}%` }}
                      />
                    ))}
                    <div className="nc-columns">
                      {insights.days.map((d) => {
                        const v = values(d);
                        const darkPct = Math.min(100, (v.dark / axisMax) * 100);
                        const totalPct = Math.min(100, (v.total / axisMax) * 100);
                        const extraPct = Math.max(0, totalPct - darkPct);
                        const isSel = selectedDate === d.date;
                        return (
                          <button
                            key={d.date}
                            className="nc-col-btn"
                            onClick={() =>
                              setSelectedDate((cur) => (cur === d.date ? null : d.date))
                            }
                          >
                            <span className="nc-bar-stack">
                              {extraPct > 0.3 && (
                                <span
                                  className="nc-bar-planned"
                                  style={{ height: `${extraPct}%` }}
                                />
                              )}
                              {darkPct > 0.3 && (
                                <span
                                  className={`nc-bar-done ${isSel ? "selected" : ""}`}
                                  style={{ height: `${darkPct}%` }}
                                />
                              )}
                            </span>
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <div className="nc-x-labels">
                    {insights.days.map((d) => {
                      const dt = new Date(d.date * 1000);
                      const wk = dt.toLocaleDateString([], { weekday: "short" });
                      const dayNum = dt.getDate();
                      const isToday = d.date === todaySec;
                      const isSel = selectedDate === d.date;
                      return (
                        <button
                          key={d.date}
                          className={`nc-x-label ${isSel || isToday ? "strong" : ""}`}
                          onClick={() =>
                            setSelectedDate((cur) => (cur === d.date ? null : d.date))
                          }
                        >
                          <span>{wk}</span>
                          <span>{dayNum}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>

              <div
                className="nc-chart-detail"
                style={{ opacity: selectedDay ? 0.85 : 0.5 }}
              >
                {detailText}
              </div>
            </div>
          )}
        </div>
      </section>

      {menuTarget && (
        <HistoryRestoreMenu
          anchor={menuTarget.rect}
          onRestore={async () => {
            await api.restoreTask(menuTarget.id);
            await fetchHistory(search, pageSize);
          }}
          onClose={() => setMenuTarget(null)}
        />
      )}
    </div>
  );
};
