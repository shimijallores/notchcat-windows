import React from "react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type { AppSnapshot, TabID } from "../types";

export const HeaderBar: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const { ui, settings, battery, sync, updateAvailable } = snapshot;

  const tabs: { id: TabID; label: string; visible: boolean }[] = [
    { id: "workspace", label: "Workspace", visible: true },
    { id: "week", label: "Week", visible: settings.showWeekTab },
    { id: "insights", label: "Insights", visible: settings.showInsightsTab },
  ];

  const syncDotClass =
    sync.phase.kind === "signed_out"
      ? "off"
      : sync.phase.kind === "syncing"
        ? "syncing"
        : sync.phase.kind === "failed" || sync.phase.kind === "revoked"
          ? "err"
          : sync.phase.kind === "offline"
            ? "warn"
            : "ok";

  return (
    <>
      <header className="nc-header">
        <div className="nc-header-left">
          <button
            className="nc-brand"
            onClick={() => api.setTab("workspace")}
            title="NotchCat"
          >
            <img src={mascotUrl} alt="NotchCat" className="nc-brand-icon" />
            <span className="nc-brand-name">NotchCat</span>
          </button>

          <nav className="nc-tabs" aria-label="Main navigation">
            {tabs
              .filter((t) => t.visible)
              .map((t) => (
                <button
                  key={t.id}
                  className={`nc-tab-btn ${ui.tab === t.id ? "active" : ""}`}
                  onClick={() => api.setTab(t.id)}
                >
                  {t.label}
                </button>
              ))}
          </nav>
        </div>

        <div className="nc-header-right">
          {updateAvailable && (
            <button
              className="nc-update-pill"
              onClick={() => api.openUrl(updateAvailable.url)}
              title={`Update ${updateAvailable.version} ready`}
            >
              ↑ v{updateAvailable.version}
            </button>
          )}

          {sync.phase.kind !== "signed_out" && (
            <button
              className="nc-sync-pill"
              onClick={() =>
                sync.phase.kind === "idle"
                  ? api.syncNow()
                  : api.setTab("settings")
              }
              title={
                sync.phase.kind === "failed"
                  ? sync.phase.message
                  : `Sync: ${sync.phase.kind}${sync.pendingCount > 0 ? ` (${sync.pendingCount} pending)` : ""}`
              }
            >
              <span className={`nc-sync-dot ${syncDotClass}`} />
              {sync.pendingCount > 0 && (
                <span className="nc-sync-pending">{sync.pendingCount}</span>
              )}
            </button>
          )}

          {settings.showBattery && battery.percent !== null && (
            <div
              className="nc-battery"
              title={battery.charging ? "Charging" : "On battery"}
            >
              <span className="nc-battery-shell">
                <span
                  className="nc-battery-fill"
                  style={{
                    width: `${Math.max(8, Math.min(100, battery.percent))}%`,
                    background:
                      battery.charging
                        ? "#6cd97c"
                        : battery.percent <= 20
                          ? "#ff6b6b"
                          : "rgba(255,255,255,0.85)",
                  }}
                />
              </span>
              <span>
                {battery.percent}%{battery.charging ? "⚡" : ""}
              </span>
            </div>
          )}

          <button
            className={`nc-icon-btn ${ui.tab === "settings" ? "active" : ""}`}
            onClick={() =>
              api.setTab(ui.tab === "settings" ? "workspace" : "settings")
            }
            title="Settings"
            aria-label="Settings"
          >
            ⚙
          </button>

          <button
            className="nc-icon-btn"
            onClick={() => api.closeTray()}
            title="Collapse (Esc)"
            aria-label="Collapse"
          >
            ✕
          </button>
        </div>
      </header>

      {ui.pendingReminder && (
        <div className="nc-inpanel-reminder">
          <div className="nc-inpanel-reminder-left">
            <span className="nc-reminder-dot" />
            <span className="nc-inpanel-reminder-title">
              Reminder: <strong>{ui.pendingReminder.title}</strong>
            </span>
          </div>
          <div className="nc-inpanel-reminder-actions">
            <button
              className="nc-mini-btn primary"
              onClick={() => api.reminderComplete(ui.pendingReminder!.id)}
            >
              Done
            </button>
            <button
              className="nc-mini-btn"
              onClick={() => api.reminderSnooze(ui.pendingReminder!.id)}
            >
              Snooze {settings.snoozeMinutes}m
            </button>
            <button
              className="nc-mini-btn"
              onClick={() => api.reminderDismiss()}
            >
              Dismiss
            </button>
          </div>
        </div>
      )}
    </>
  );
};
