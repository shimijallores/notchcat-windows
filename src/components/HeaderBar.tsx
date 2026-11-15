import React from "react";
import {
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  BatteryMedium,
  Check,
  CircleArrowDown,
  Settings,
  X,
} from "lucide-react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type { AppSnapshot, TabID } from "../types";

export const ReminderButtons: React.FC<{
  snoozeMinutes: number;
  onSnooze: () => void;
  onDone: () => void;
}> = ({ snoozeMinutes, onSnooze, onDone }) => (
  <div className="nc-reminder-buttons">
    <button className="nc-snooze" onClick={onSnooze}>
      <b>zZ</b>
      <span>{snoozeMinutes} min</span>
    </button>
    <button className="nc-done" onClick={onDone}>
      <Check size={11} strokeWidth={3.5} />
      <span>Done</span>
    </button>
  </div>
);

const BatteryView: React.FC<{ percent: number; charging: boolean }> = ({
  percent,
  charging,
}) => {
  const low = percent <= 20 && !charging;
  const Icon = charging
    ? BatteryCharging
    : percent >= 88
      ? BatteryFull
      : percent >= 38
        ? BatteryMedium
        : BatteryLow;
  const color = low
    ? "rgb(255,107,102)"
    : charging
      ? "rgb(115,230,140)"
      : "rgba(255,255,255,0.7)";
  return (
    <span
      className="nc-battery"
      style={{ color }}
      title={charging ? "Charging" : "On battery"}
    >
      <Icon size={16} strokeWidth={1.8} />
      <span>{percent}%</span>
    </span>
  );
};

export const HeaderBar: React.FC<{ snapshot: AppSnapshot }> = ({ snapshot }) => {
  const { ui, settings, battery, sync, updateAvailable } = snapshot;

  const tabs: { id: TabID; label: string; visible: boolean }[] = [
    { id: "workspace", label: "Workspace", visible: true },
    { id: "week", label: "Week", visible: settings.showWeekTab },
    { id: "insights", label: "Insights", visible: settings.showInsightsTab },
  ];
  const syncProblem =
    sync.phase.kind === "failed" ||
    sync.phase.kind === "revoked" ||
    sync.phase.kind === "offline";

  return (
    <>
      <header className="nc-header">
        <div className="nc-header-left">
          <img src={mascotUrl} alt="" className="nc-brand-icon" />
          <span className="nc-brand-name">NotchCat</span>
          {settings.showBattery && battery.percent !== null && (
            <BatteryView percent={battery.percent} charging={battery.charging} />
          )}
          {settings.checkForUpdates && updateAvailable && (
            <button
              className="nc-update-pill"
              onClick={() => {
                api.openUrl(updateAvailable.url);
                api.closeTray();
              }}
              title={`NotchCat ${updateAvailable.version} is out. Opens the download page on GitHub.`}
            >
              <CircleArrowDown size={11} fill="currentColor" stroke="var(--amber)" />
              Update available
            </button>
          )}
        </div>

        <div className="nc-notch-gap" />

        <nav className="nc-header-right" aria-label="Main navigation">
          {syncProblem && (
            <button
              className="nc-sync-warn"
              onClick={() => api.setTab("settings")}
              title={
                sync.phase.kind === "failed"
                  ? sync.phase.message
                  : `Sync ${sync.phase.kind}`
              }
            />
          )}
          {tabs
            .filter((t) => t.visible)
            .map((t) => (
              <button
                key={t.id}
                className={`nc-tab ${ui.tab === t.id ? "active" : ""}`}
                onClick={() => api.setTab(t.id)}
              >
                {t.label}
              </button>
            ))}
          <button
            className={`nc-gear ${ui.tab === "settings" ? "active" : ""}`}
            onClick={() => api.setTab("settings")}
            title="Settings"
            aria-label="Settings"
          >
            <Settings size={13} fill="currentColor" strokeWidth={2} />
          </button>
        </nav>
      </header>

      {ui.pendingReminder && (
        <div className="nc-banner">
          <img src={mascotUrl} alt="" width={26} height={26} />
          <div className="nc-banner-text">
            <b>{ui.pendingReminder.title}</b>
            <span>Reminder</span>
          </div>
          <span style={{ flex: 1 }} />
          <ReminderButtons
            snoozeMinutes={settings.snoozeMinutes}
            onSnooze={() => api.reminderSnooze(ui.pendingReminder!.id)}
            onDone={() => api.reminderComplete(ui.pendingReminder!.id)}
          />
          <button
            className="nc-banner-x"
            onClick={() => api.reminderDismiss()}
            title="Dismiss"
            aria-label="Dismiss"
          >
            <X size={12} strokeWidth={2.5} />
          </button>
        </div>
      )}
    </>
  );
};
