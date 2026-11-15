import React, { useEffect, useState } from "react";
import { api } from "../api";
import type { AppSnapshot, HistoryPage } from "../types";
import { formatDurationShort } from "../useNotchCat";

export const InsightsView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const [subTab, setSubTab] = useState<"insights" | "history">("insights");
  const [search, setSearch] = useState("");
  const [pageSize, setPageSize] = useState(60);
  const [history, setHistory] = useState<HistoryPage | null>(null);

  const { insights, ui } = snapshot;
  const maxCompleted = Math.max(1, ...insights.days.map((d) => d.completed));
  const maxFocus = Math.max(60, ...insights.days.map((d) => d.focusSeconds));

  const fetchHistory = async (q: string, size: number) => {
    const res = await api.getHistory(q, size);
    setHistory(res);
  };

  useEffect(() => {
    if (subTab === "history") {
      fetchHistory(search, pageSize);
    }
  }, [subTab, search, pageSize, snapshot.todayTasks.length]);

  const weekLabel = (() => {
    if (ui.insightsWeekOffset === 0) return "This week";
    if (ui.insightsWeekOffset === -1) return "Last week";
    const first = insights.days[0]?.date;
    const last = insights.days[insights.days.length - 1]?.date;
    if (!first || !last) return "";
    const fmt = (s: number) =>
      new Date(s * 1000).toLocaleDateString([], {
        month: "short",
        day: "numeric",
      });
    return `${fmt(first)} – ${fmt(last)}`;
  })();

  return (
    <div className="nc-insights-wrap">
      <div className="nc-insights-topbar">
        <div className="nc-seg-dark">
          <button
            className={subTab === "insights" ? "active" : ""}
            onClick={() => setSubTab("insights")}
          >
            Summary
          </button>
          <button
            className={subTab === "history" ? "active" : ""}
            onClick={() => setSubTab("history")}
          >
            History
          </button>
        </div>

        {subTab === "insights" ? (
          <div className="nc-week-nav">
            <button
              className="nc-mini-dark-btn"
              onClick={() =>
                api.setInsightsWeekOffset(ui.insightsWeekOffset - 1)
              }
              title="Previous 7 days"
            >
              ‹
            </button>
            <span className="nc-week-nav-label">{weekLabel}</span>
            <button
              className="nc-mini-dark-btn"
              disabled={ui.insightsWeekOffset >= 0}
              onClick={() =>
                api.setInsightsWeekOffset(ui.insightsWeekOffset + 1)
              }
              title="Next 7 days"
            >
              ›
            </button>
          </div>
        ) : (
          <input
            className="nc-search-input"
            placeholder="Search finished tasks…"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setPageSize(60);
            }}
          />
        )}
      </div>

      {subTab === "insights" ? (
        <div className="nc-insights-body">
          <div className="nc-insights-left">
            <div className="nc-insights-headline">{insights.headline}</div>
            <p className="nc-insights-sentence">{insights.sentence}</p>

            <div className="nc-stats-grid">
              <div className="nc-stat-tile mint">
                <span className="nc-stat-val">{insights.completed}</span>
                <span className="nc-stat-lbl">Completed</span>
              </div>
              <div className="nc-stat-tile lavender">
                <span className="nc-stat-val">
                  {formatDurationShort(insights.focusSeconds)}
                </span>
                <span className="nc-stat-lbl">Focused</span>
              </div>
              <div className="nc-stat-tile sand">
                <span className="nc-stat-val">{insights.planned}</span>
                <span className="nc-stat-lbl">Planned</span>
              </div>
              <div className="nc-stat-tile slate">
                <span className="nc-stat-val">{insights.activeDays}/7</span>
                <span className="nc-stat-lbl">Active days</span>
              </div>
            </div>
          </div>

          <div className="nc-insights-chart-card">
            <div className="nc-chart-header">
              <span>7-Day Activity</span>
              <div className="nc-chart-legend">
                <span>
                  <i className="dot focus" /> Focus
                </span>
                <span>
                  <i className="dot tasks" /> Tasks
                </span>
              </div>
            </div>

            <div className="nc-bars-row">
              {insights.days.map((d) => {
                const dateObj = new Date(d.date * 1000);
                const dayShort = dateObj.toLocaleDateString([], {
                  weekday: "short",
                });
                const focusPct = Math.round((d.focusSeconds / maxFocus) * 100);
                const tasksPct = Math.round((d.completed / maxCompleted) * 100);
                return (
                  <div
                    key={d.date}
                    className="nc-bar-col"
                    title={`${dayShort}: ${d.completed} done, ${formatDurationShort(d.focusSeconds)} focused`}
                  >
                    <div className="nc-bar-pair">
                      <div
                        className="nc-bar focus"
                        style={{ height: `${Math.max(4, focusPct)}%` }}
                      />
                      <div
                        className="nc-bar tasks"
                        style={{ height: `${Math.max(4, tasksPct)}%` }}
                      />
                    </div>
                    <span className="nc-bar-label">{dayShort}</span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      ) : (
        <div className="nc-history-scroll">
          {!history || history.sections.length === 0 ? (
            <div className="nc-empty-dark">
              {search.trim()
                ? `No completed tasks matching "${search.trim()}"`
                : "Completed tasks will appear here."}
            </div>
          ) : (
            <>
              {history.sections.map((sec) => (
                <div key={sec.day} className="nc-hist-section">
                  <div className="nc-hist-sec-head">
                    <span>{sec.title}</span>
                    {sec.focusSeconds > 0 && (
                      <span className="nc-hist-focus">
                        {formatDurationShort(sec.focusSeconds)}
                      </span>
                    )}
                  </div>
                  {sec.items.map((item) => (
                    <div key={item.id} className="nc-hist-row">
                      <span className="nc-hist-check">✓</span>
                      <span className="nc-hist-title">{item.title}</span>
                      {item.focusSeconds > 0 && (
                        <span className="nc-hist-dur">
                          {formatDurationShort(item.focusSeconds)}
                        </span>
                      )}
                      <button
                        className="nc-mini-dark-btn"
                        onClick={async () => {
                          await api.restoreTask(item.id);
                          await fetchHistory(search, pageSize);
                        }}
                      >
                        Restore to today
                      </button>
                    </div>
                  ))}
                </div>
              ))}
              {history.canLoadMore && (
                <button
                  className="nc-load-more-btn"
                  onClick={() => setPageSize((s) => s + 60)}
                >
                  Load more ({history.total - history.sections.reduce((a, b) => a + b.items.length, 0)} left)
                </button>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
};
