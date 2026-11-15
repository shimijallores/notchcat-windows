import React, { useState } from "react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type {
  AppSnapshot,
  CardID,
  FakeNotchStyle,
  SettingsData,
} from "../types";

type SettingsSection = "cards" | "opening" | "timer" | "account";

const CARD_NAMES: Record<CardID, string> = {
  tasks: "Tasks",
  timer: "Focus Timer",
  notepad: "Notepad",
  events: "Events & Reminders",
  music: "Now Playing",
};

const AccountPanel: React.FC<{ snapshot: AppSnapshot }> = ({ snapshot }) => {
  const { sync } = snapshot;
  const [serverUrl, setServerUrl] = useState(
    sync.serverUrl || "https://api.notchcat.cloud",
  );
  const [mode, setMode] = useState<"signin" | "signup">("signin");
  const [signupStep, setSignupStep] = useState<"key" | "details">("key");
  const [accessKey, setAccessKey] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [welcomeName, setWelcomeName] = useState<string | null>(null);

  if (welcomeName) {
    return (
      <div className="nc-welcome-card">
        <img src={mascotUrl} alt="" className="nc-welcome-icon" />
        <div className="nc-welcome-title">Welcome, {welcomeName}!</div>
        <p className="nc-welcome-sub">
          Your tasks, notepad, focus history, and preferences now stay in sync
          across your devices.
        </p>
        <button
          className="nc-accent-btn"
          onClick={() => setWelcomeName(null)}
        >
          Continue
        </button>
      </div>
    );
  }

  if (sync.phase.kind !== "signed_out") {
    const phaseText =
      sync.phase.kind === "idle"
        ? "Connected & up to date"
        : sync.phase.kind === "syncing"
          ? "Syncing…"
          : sync.phase.kind === "offline"
            ? "Offline (changes queued locally)"
            : sync.phase.kind === "revoked"
              ? "Access key revoked"
              : sync.phase.message;

    return (
      <div className="nc-account-signedin">
        <div className="nc-account-hero">
          <img src={mascotUrl} alt="" className="nc-account-avatar" />
          <div>
            <div className="nc-account-user">
              {sync.username ?? "NotchCat User"}
            </div>
            <div className="nc-account-server">{sync.serverUrl}</div>
          </div>
          <span className={`nc-status-badge ${sync.phase.kind}`}>
            {phaseText}
          </span>
        </div>

        <div className="nc-account-meta-row">
          <span>
            Last sync:{" "}
            {sync.lastSyncAt
              ? new Date(sync.lastSyncAt).toLocaleTimeString([], {
                  hour: "numeric",
                  minute: "2-digit",
                })
              : "Never"}
          </span>
          <span>Pending edits: {sync.pendingCount}</span>
        </div>

        <div className="nc-row-gap">
          <button
            className="nc-accent-btn"
            disabled={sync.phase.kind === "syncing" || busy}
            onClick={async () => {
              setBusy(true);
              try {
                await api.syncNow();
              } finally {
                setBusy(false);
              }
            }}
          >
            {sync.phase.kind === "syncing" ? "Syncing…" : "Sync now"}
          </button>
          <button
            className="nc-mini-dark-btn danger"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await api.syncSignOut();
              } finally {
                setBusy(false);
              }
            }}
          >
            Sign out
          </button>
        </div>
      </div>
    );
  }

  const verifyKey = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      await api.syncCheckKey(serverUrl, accessKey);
      setSignupStep("details");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const submitSignUp = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      await api.syncSignUp(serverUrl, accessKey, username, password);
      setWelcomeName(username.trim());
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const submitSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      await api.syncSignIn(serverUrl, username, password);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="nc-account-form-wrap">
      <div className="nc-seg-dark">
        <button
          className={mode === "signin" ? "active" : ""}
          onClick={() => {
            setMode("signin");
            setError(null);
          }}
        >
          Sign in
        </button>
        <button
          className={mode === "signup" ? "active" : ""}
          onClick={() => {
            setMode("signup");
            setSignupStep("key");
            setError(null);
          }}
        >
          Create account
        </button>
      </div>

      <label className="nc-field-label">
        <span>Sync Server URL</span>
        <input
          className="nc-dark-input"
          value={serverUrl}
          onChange={(e) => setServerUrl(e.target.value)}
          onBlur={() => api.syncSetServerUrl(serverUrl)}
          placeholder="https://api.notchcat.cloud"
        />
      </label>

      {error && <div className="nc-error-banner">{error}</div>}

      {mode === "signin" ? (
        <form className="nc-form-col" onSubmit={submitSignIn}>
          <input
            className="nc-dark-input"
            placeholder="Username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
          <input
            className="nc-dark-input"
            type="password"
            placeholder="Password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
          />
          <button type="submit" className="nc-accent-btn" disabled={busy}>
            {busy ? "Signing in…" : "Sign in"}
          </button>
        </form>
      ) : signupStep === "key" ? (
        <form className="nc-form-col" onSubmit={verifyKey}>
          <input
            className="nc-dark-input"
            placeholder="Access key"
            value={accessKey}
            onChange={(e) => setAccessKey(e.target.value)}
            required
          />
          <button type="submit" className="nc-accent-btn" disabled={busy}>
            {busy ? "Checking…" : "Verify access key"}
          </button>
        </form>
      ) : (
        <form className="nc-form-col" onSubmit={submitSignUp}>
          <input
            className="nc-dark-input"
            placeholder="Username (3–32 chars: a-z, 0-9, . _ -)"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            required
          />
          <input
            className="nc-dark-input"
            type="password"
            placeholder="Password (min 8 chars)"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            minLength={8}
            required
          />
          <div className="nc-row-gap">
            <button
              type="button"
              className="nc-mini-dark-btn"
              onClick={() => setSignupStep("key")}
            >
              Back
            </button>
            <button type="submit" className="nc-accent-btn" disabled={busy}>
              {busy ? "Creating…" : "Create account"}
            </button>
          </div>
        </form>
      )}
    </div>
  );
};

export const SettingsView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const [section, setSection] = useState<SettingsSection>("cards");
  const { settings } = snapshot;
  const [icsDraft, setIcsDraft] = useState(settings.icsUrl ?? "");
  const [backupMsg, setBackupMsg] = useState<string | null>(null);

  const patch = (next: Partial<SettingsData>) => {
    api.settingsUpdate({ ...settings, ...next });
  };

  const enabledCount = settings.cards.filter((c) => c.enabled).length;

  return (
    <div className="nc-settings-shell">
      <aside className="nc-settings-nav">
        <button
          className={section === "cards" ? "active" : ""}
          onClick={() => setSection("cards")}
        >
          Cards & Tabs
        </button>
        <button
          className={section === "opening" ? "active" : ""}
          onClick={() => setSection("opening")}
        >
          Island & Opening
        </button>
        <button
          className={section === "timer" ? "active" : ""}
          onClick={() => setSection("timer")}
        >
          Timer & System
        </button>
        <button
          className={section === "account" ? "active" : ""}
          onClick={() => setSection("account")}
        >
          Cloud Sync
        </button>
      </aside>

      <div className="nc-settings-body">
        {section === "cards" && (
          <div className="nc-settings-list">
            <div className="nc-settings-group-title">Workspace Cards</div>
            {settings.cards.map((item, idx) => (
              <div key={item.card} className="nc-setting-row">
                <label className="nc-toggle-label">
                  <input
                    type="checkbox"
                    checked={item.enabled}
                    disabled={item.enabled && enabledCount <= 1}
                    onChange={(e) =>
                      api.settingsSetCardEnabled(item.card, e.target.checked)
                    }
                  />
                  <span>{CARD_NAMES[item.card]}</span>
                </label>
                <div className="nc-row-gap">
                  <button
                    className="nc-mini-dark-btn"
                    disabled={idx === 0}
                    onClick={() => api.settingsMoveCard(item.card, -1)}
                  >
                    ↑
                  </button>
                  <button
                    className="nc-mini-dark-btn"
                    disabled={idx === settings.cards.length - 1}
                    onClick={() => api.settingsMoveCard(item.card, 1)}
                  >
                    ↓
                  </button>
                </div>
              </div>
            ))}

            <div className="nc-settings-group-title">Tabs & Look</div>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show Week planner tab</span>
              <input
                type="checkbox"
                checked={settings.showWeekTab}
                onChange={(e) => patch({ showWeekTab: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show Insights & History tab</span>
              <input
                type="checkbox"
                checked={settings.showInsightsTab}
                onChange={(e) => patch({ showInsightsTab: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Tactile paper grain on cards</span>
              <input
                type="checkbox"
                checked={settings.paperGrain}
                onChange={(e) => patch({ paperGrain: e.target.checked })}
              />
            </label>
          </div>
        )}

        {section === "opening" && (
          <div className="nc-settings-list">
            <div className="nc-settings-group-title">Top Island Style</div>
            <div className="nc-seg-dark">
              {(["island", "notch", "off"] as FakeNotchStyle[]).map((st) => (
                <button
                  key={st}
                  className={settings.fakeNotch === st ? "active" : ""}
                  onClick={() => patch({ fakeNotch: st })}
                >
                  {st === "island"
                    ? "Floating Island"
                    : st === "notch"
                      ? "Screen Notch"
                      : "Off"}
                </button>
              ))}
            </div>

            <div className="nc-settings-group-title">Opening Behaviour</div>
            <label className="nc-setting-row nc-toggle-label">
              <span>Hover top island to open</span>
              <input
                type="checkbox"
                checked={settings.hoverToOpen}
                onChange={(e) => patch({ hoverToOpen: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Click island to open & pin</span>
              <input
                type="checkbox"
                checked={settings.clickToOpen}
                onChange={(e) => patch({ clickToOpen: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Global shortcut (Alt+N)</span>
              <input
                type="checkbox"
                checked={settings.hotkeyEnabled}
                onChange={(e) => patch({ hotkeyEnabled: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Launch NotchCat at Windows login</span>
              <input
                type="checkbox"
                checked={snapshot.launchAtLogin}
                onChange={(e) => api.settingsSetLaunchAtLogin(e.target.checked)}
              />
            </label>

            <div className="nc-settings-group-title">Collapsed Chips & Status</div>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show collapsed timer pill</span>
              <input
                type="checkbox"
                checked={settings.showPill}
                onChange={(e) => patch({ showPill: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show collapsed music chip</span>
              <input
                type="checkbox"
                checked={settings.showMusicChip}
                onChange={(e) => patch({ showMusicChip: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show laptop battery in header</span>
              <input
                type="checkbox"
                checked={settings.showBattery}
                onChange={(e) => patch({ showBattery: e.target.checked })}
              />
            </label>
          </div>
        )}

        {section === "timer" && (
          <div className="nc-settings-list">
            <div className="nc-settings-group-title">Focus & Reminders</div>
            <div className="nc-setting-row">
              <span>Default focus length</span>
              <div className="nc-row-gap">
                <button
                  className="nc-mini-dark-btn"
                  onClick={() =>
                    patch({
                      defaultFocusMinutes: Math.max(
                        5,
                        settings.defaultFocusMinutes - 5,
                      ),
                    })
                  }
                >
                  −
                </button>
                <span className="nc-stepper-val">
                  {settings.defaultFocusMinutes}m
                </span>
                <button
                  className="nc-mini-dark-btn"
                  onClick={() =>
                    patch({
                      defaultFocusMinutes: Math.min(
                        240,
                        settings.defaultFocusMinutes + 5,
                      ),
                    })
                  }
                >
                  +
                </button>
              </div>
            </div>

            <label className="nc-setting-row nc-toggle-label">
              <span>Play chime when timer or reminder fires</span>
              <input
                type="checkbox"
                checked={settings.timerSound}
                onChange={(e) => patch({ timerSound: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show Time&apos;s Up banner when collapsed</span>
              <input
                type="checkbox"
                checked={settings.timeUpIsland}
                onChange={(e) => patch({ timeUpIsland: e.target.checked })}
              />
            </label>
            <label className="nc-setting-row nc-toggle-label">
              <span>Enable task reminders</span>
              <input
                type="checkbox"
                checked={settings.remindersEnabled}
                onChange={(e) => patch({ remindersEnabled: e.target.checked })}
              />
            </label>
            <div className="nc-setting-row">
              <span>Reminder snooze length</span>
              <div className="nc-row-gap">
                <button
                  className="nc-mini-dark-btn"
                  onClick={() =>
                    patch({
                      snoozeMinutes: Math.max(1, settings.snoozeMinutes - 5),
                    })
                  }
                >
                  −
                </button>
                <span className="nc-stepper-val">{settings.snoozeMinutes}m</span>
                <button
                  className="nc-mini-dark-btn"
                  onClick={() =>
                    patch({
                      snoozeMinutes: Math.min(60, settings.snoozeMinutes + 5),
                    })
                  }
                >
                  +
                </button>
              </div>
            </div>
            <label className="nc-setting-row nc-toggle-label">
              <span>Show scheduled reminders inside Events card</span>
              <input
                type="checkbox"
                checked={settings.showRemindersInEvents}
                onChange={(e) =>
                  patch({ showRemindersInEvents: e.target.checked })
                }
              />
            </label>

            <div className="nc-settings-group-title">Calendar ICS Feed</div>
            <div className="nc-row-gap">
              <input
                className="nc-dark-input"
                placeholder="https://.../basic.ics"
                value={icsDraft}
                onChange={(e) => setIcsDraft(e.target.value)}
              />
              <button
                className="nc-mini-dark-btn"
                onClick={() => patch({ icsUrl: icsDraft.trim() || null })}
              >
                Save
              </button>
            </div>

            <div className="nc-settings-group-title">Data & Updates (v{snapshot.version})</div>
            <div className="nc-row-gap wrap">
              <button
                className="nc-mini-dark-btn"
                onClick={async () => {
                  const p = await api.backupDatabase();
                  setBackupMsg(`Backed up to ${p}`);
                }}
              >
                Backup Database
              </button>
              <button
                className="nc-mini-dark-btn"
                onClick={() => api.revealDataFolder()}
              >
                Open Data Folder
              </button>
              <button
                className="nc-mini-dark-btn"
                onClick={() => api.updateCheck()}
              >
                Check Updates
              </button>
              <button
                className="nc-mini-dark-btn danger"
                onClick={() => api.quitApp()}
              >
                Quit NotchCat
              </button>
            </div>
            {backupMsg && <div className="nc-hint-green">{backupMsg}</div>}
            {snapshot.updateStatusMessage && (
              <div className="nc-hint-muted">
                {snapshot.updateStatusMessage}
              </div>
            )}
          </div>
        )}

        {section === "account" && <AccountPanel snapshot={snapshot} />}
      </div>
    </div>
  );
};
