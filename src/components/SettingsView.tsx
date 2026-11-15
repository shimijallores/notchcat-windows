import React, { useEffect, useState } from "react";
import {
  AlertCircle,
  ArrowDownCircle,
  BatteryMedium,
  Bell,
  Calendar,
  CalendarDays,
  ChartBar,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Clock,
  Command,
  Eye,
  EyeOff,
  Folder,
  Grid3x3,
  HardDrive,
  List,
  Minus,
  MousePointer,
  Music2,
  NotebookText,
  PanelTop,
  Plus,
  Power,
  RectangleHorizontal,
  Timer,
  Volume2,
  XCircle,
} from "lucide-react";
import mascotUrl from "../assets/mascot.png";
import { api } from "../api";
import type {
  AppSnapshot,
  CardID,
  FakeNotchStyle,
  SettingsData,
} from "../types";

type AuthFlow =
  | null
  | { kind: "signIn" }
  | { kind: "signUp" }
  | { kind: "welcome"; name: string; created: boolean };

const CARD_META: Record<CardID, { title: string; icon: React.ReactNode }> = {
  tasks: { title: "Tasks", icon: <CheckCircle2 size={12} strokeWidth={2.2} /> },
  timer: { title: "Focus timer", icon: <Timer size={12} strokeWidth={2.2} /> },
  notepad: {
    title: "Notepad",
    icon: <NotebookText size={12} strokeWidth={2.2} />,
  },
  events: { title: "Events", icon: <Calendar size={12} strokeWidth={2.2} /> },
  music: { title: "Now playing", icon: <Music2 size={12} strokeWidth={2.2} /> },
};

const AmberSwitch: React.FC<{
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}> = ({ checked, disabled, onChange }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    disabled={disabled}
    className={`nc-amber-switch ${checked ? "on" : ""}`}
    onClick={() => !disabled && onChange(!checked)}
  >
    <span className="nc-amber-thumb" />
  </button>
);

const Section: React.FC<{
  title: string;
  footer?: string | null;
  children: React.ReactNode;
}> = ({ title, footer, children }) => (
  <div className="nc-settings-card">
    <div className="nc-settings-card-title">{title}</div>
    <div className="nc-settings-card-rows">{children}</div>
    {footer && <div className="nc-settings-card-footer">{footer}</div>}
  </div>
);

const ToggleRow: React.FC<{
  title: string;
  icon: React.ReactNode;
  checked: boolean;
  onChange: (next: boolean) => void;
}> = ({ title, icon, checked, onChange }) => (
  <div className="nc-setting-row">
    <span className="nc-setting-ico">{icon}</span>
    <span className="nc-setting-lbl">{title}</span>
    <AmberSwitch checked={checked} onChange={onChange} />
  </div>
);

const StepperRow: React.FC<{
  title: string;
  icon: React.ReactNode;
  value: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  onChange: (next: number) => void;
}> = ({ title, icon, value, min, max, step, unit, onChange }) => (
  <div className="nc-setting-row">
    <span className="nc-setting-ico">{icon}</span>
    <span className="nc-setting-lbl">{title}</span>
    <div className="nc-set-stepper">
      <button
        className="nc-set-arrow"
        disabled={value <= min}
        onClick={() => onChange(Math.max(min, value - step))}
      >
        <Minus size={10} strokeWidth={3} />
      </button>
      <span className="nc-set-val">
        {value} {unit}
      </span>
      <button
        className="nc-set-arrow"
        disabled={value >= max}
        onClick={() => onChange(Math.min(max, value + step))}
      >
        <Plus size={10} strokeWidth={3} />
      </button>
    </div>
  </div>
);

const ActionRow: React.FC<{
  title: string;
  icon: React.ReactNode;
  onClick: () => void;
}> = ({ title, icon, onClick }) => (
  <button className="nc-setting-row action" onClick={onClick}>
    <span className="nc-setting-ico">{icon}</span>
    <span className="nc-setting-lbl">{title}</span>
    <ChevronRight size={11} strokeWidth={2.8} style={{ opacity: 0.35 }} />
  </button>
);

// ---------------------------------------------------------------------------
// Cloud sync card inside Settings list
// ---------------------------------------------------------------------------

function relativeSyncTime(ms: number): string {
  const diffSec = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (diffSec < 45) return "just now";
  const mins = Math.round(diffSec / 60);
  if (mins < 60) return `${mins}m ago`;
  return new Date(ms).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

const CloudSyncPanel: React.FC<{
  snapshot: AppSnapshot;
  onStartFlow: (f: AuthFlow) => void;
}> = ({ snapshot, onStartFlow }) => {
  const { sync } = snapshot;
  const [busy, setBusy] = useState(false);

  if (sync.phase.kind !== "signed_out" && sync.username) {
    const dotClass =
      sync.phase.kind === "idle"
        ? "ok"
        : sync.phase.kind === "syncing"
          ? "syncing"
          : sync.phase.kind === "offline"
            ? "off"
            : "err";
    const statusText =
      sync.phase.kind === "syncing"
        ? "Syncing…"
        : sync.phase.kind === "offline"
          ? "Offline. Changes sync when you reconnect."
          : sync.phase.kind === "revoked"
            ? "Cloud access was turned off. Your tasks are still here."
            : sync.phase.kind === "failed"
              ? sync.phase.message
              : sync.lastSyncAt
                ? `Synced ${relativeSyncTime(sync.lastSyncAt)}`
                : "Signed in";

    return (
      <div className="nc-sync-summary">
        <div className="nc-sync-user-row">
          <span className="nc-avatar-circle">
            {sync.username.slice(0, 1).toUpperCase()}
          </span>
          <div className="nc-sync-user-meta">
            <b>{sync.username}</b>
            <span className="nc-sync-status-line">
              <i className={`nc-dot-sm ${dotClass}`} />
              <span>{statusText}</span>
            </span>
          </div>
        </div>
        <div className="nc-sync-btn-row">
          {sync.phase.kind !== "revoked" && (
            <button
              className="nc-amber-pill-btn"
              disabled={busy || sync.phase.kind === "syncing"}
              onClick={async () => {
                setBusy(true);
                try {
                  await api.syncNow();
                } finally {
                  setBusy(false);
                }
              }}
            >
              Sync now
            </button>
          )}
          <button
            className="nc-signout-link"
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

  return (
    <div className="nc-choice-row">
      <button
        className="nc-choice-btn mint"
        onClick={() => onStartFlow({ kind: "signIn" })}
      >
        Sign in
      </button>
      <button
        className="nc-choice-btn lavender"
        onClick={() => onStartFlow({ kind: "signUp" })}
      >
        Sign up
      </button>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Auth full-card takeover (AccountView.swift)
// ---------------------------------------------------------------------------

const WelcomeScreen: React.FC<{
  name: string;
  created: boolean;
  tint: string;
  onFinish: () => void;
}> = ({ name, created, tint, onFinish }) => {
  const [stage, setStage] = useState(0);

  useEffect(() => {
    const t0 = window.setTimeout(() => setStage(1), 40);
    const t1 = window.setTimeout(() => setStage(2), 340);
    const t2 = window.setTimeout(() => setStage(3), 580);
    const t3 = window.setTimeout(onFinish, 2000);
    return () => {
      window.clearTimeout(t0);
      window.clearTimeout(t1);
      window.clearTimeout(t2);
      window.clearTimeout(t3);
    };
  }, [onFinish]);

  const circumference = 2 * Math.PI * 52;

  return (
    <div className="nc-welcome">
      <div className="nc-welcome-badge">
        <svg width="112" height="112" viewBox="0 0 112 112">
          <circle
            cx="56"
            cy="56"
            r="52"
            fill="none"
            stroke="rgba(27,29,28,0.1)"
            strokeWidth="4"
          />
          <circle
            cx="56"
            cy="56"
            r="52"
            fill="none"
            stroke="var(--ink)"
            strokeWidth="4"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={stage >= 1 ? 0 : circumference}
            transform="rotate(-90 56 56)"
            style={{ transition: "stroke-dashoffset 550ms ease-out" }}
          />
        </svg>
        <img
          src={mascotUrl}
          alt=""
          className={`nc-welcome-cat ${stage >= 2 ? "shown" : ""}`}
        />
        <span
          className={`nc-welcome-check ${stage >= 2 ? "shown" : ""}`}
          style={{ borderColor: tint }}
        >
          <Check size={13} strokeWidth={3.5} />
        </span>
      </div>

      <div className={`nc-welcome-text ${stage >= 3 ? "shown" : ""}`}>
        <h2>{created ? `You're in, ${name}` : `Welcome back, ${name}`}</h2>
        <p>Your tasks, notes and focus time now follow you everywhere.</p>
      </div>
    </div>
  );
};

const AuthTakeover: React.FC<{
  snapshot: AppSnapshot;
  flow: Exclude<AuthFlow, null>;
  grain: boolean;
  onFlowChange: (f: AuthFlow) => void;
}> = ({ snapshot, flow, grain, onFlowChange }) => {
  const [step, setStep] = useState<"key" | "details">("key");
  const [serverUrl, setServerUrl] = useState(
    snapshot.sync.serverUrl || "https://api.notchcat.cloud",
  );
  const [showServer, setShowServer] = useState(false);
  const [accessKey, setAccessKey] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const tint =
    flow.kind === "signIn"
      ? "var(--mint)"
      : flow.kind === "signUp"
        ? "var(--lavender)"
        : flow.created
          ? "var(--lavender)"
          : "var(--mint)";

  if (flow.kind === "welcome") {
    return (
      <section
        className="nc-paper-card nc-auth-takeover"
        style={{ background: tint }}
      >
        {grain && <div className="nc-paper-grain" aria-hidden="true" />}
        <div className="nc-card-body">
          <WelcomeScreen
            name={flow.name}
            created={flow.created}
            tint={tint}
            onFinish={() => onFlowChange(null)}
          />
        </div>
      </section>
    );
  }

  const isSignIn = flow.kind === "signIn";
  const title = isSignIn
    ? "Welcome back"
    : step === "key"
      ? "Join NotchCat"
      : "Almost there";
  const subtitle = isSignIn
    ? "Sign in to keep your tasks, notes and focus time in step on every device."
    : step === "key"
      ? "Cloud sync is invite only. Enter the access key you were given."
      : "Choose a username and a password for your account.";

  const sentence = (msg: string) => (/[.?!]$/.test(msg) ? msg : `${msg}.`);

  const verifyKey = async () => {
    if (working || !accessKey.trim()) return;
    setWorking(true);
    setError(null);
    try {
      await api.syncCheckKey(serverUrl, accessKey.trim());
      setStep("details");
    } catch (e) {
      setError(sentence(String(e)));
    } finally {
      setWorking(false);
    }
  };

  const submit = async () => {
    if (working) return;
    setWorking(true);
    setError(null);
    try {
      if (isSignIn) {
        if (!username.trim() || !password) return;
        await api.syncSignIn(serverUrl, username.trim(), password);
        onFlowChange({
          kind: "welcome",
          name: username.trim(),
          created: false,
        });
      } else {
        if (step !== "details" || !username.trim() || password.length < 8)
          return;
        await api.syncSignUp(
          serverUrl,
          accessKey.trim(),
          username.trim(),
          password,
        );
        onFlowChange({ kind: "welcome", name: username.trim(), created: true });
      }
    } catch (e) {
      setError(sentence(String(e)));
    } finally {
      setWorking(false);
    }
  };

  return (
    <section
      className="nc-paper-card nc-auth-takeover"
      style={{ background: tint }}
    >
      {grain && <div className="nc-paper-grain" aria-hidden="true" />}
      <div className="nc-card-body">
        <button className="nc-back-pill" onClick={() => onFlowChange(null)}>
          <ChevronLeft size={11} strokeWidth={3} />
          Settings
        </button>

        <div className="nc-auth-split">
          <div className="nc-auth-intro">
            <img src={mascotUrl} alt="" width={54} height={54} />
            <h1>{title}</h1>
            <p>{subtitle}</p>
          </div>

          <div className="nc-auth-form">
            {isSignIn ? (
              <>
                <div className="nc-field">
                  <label>Username</label>
                  <div className={`nc-field-box ${error ? "invalid" : ""}`}>
                    <input
                      value={username}
                      placeholder="alex"
                      autoFocus
                      onChange={(e) => setUsername(e.target.value)}
                    />
                  </div>
                </div>
                <div className="nc-field">
                  <label>Password</label>
                  <div className={`nc-field-box ${error ? "invalid" : ""}`}>
                    <input
                      type={showPassword ? "text" : "password"}
                      value={password}
                      placeholder="Password"
                      onChange={(e) => setPassword(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && submit()}
                    />
                    <button
                      type="button"
                      className="nc-eye-btn"
                      onClick={() => setShowPassword((v) => !v)}
                      aria-label={
                        showPassword ? "Hide password" : "Show password"
                      }
                    >
                      {showPassword ? <EyeOff size={13} /> : <Eye size={13} />}
                    </button>
                  </div>
                </div>
                <div className="nc-auth-msg">
                  {error ? (
                    <span className="nc-auth-err">
                      <AlertCircle size={12} />
                      <span>{error}</span>
                    </span>
                  ) : showServer ? (
                    <input
                      className="nc-server-inline"
                      value={serverUrl}
                      onChange={(e) => setServerUrl(e.target.value)}
                      onBlur={() => api.syncSetServerUrl(serverUrl)}
                      placeholder="https://api.notchcat.cloud"
                    />
                  ) : (
                    <button
                      type="button"
                      className="nc-linklike"
                      onClick={() => setShowServer(true)}
                    >
                      Server: {serverUrl.replace(/^https?:\/\//, "")}
                    </button>
                  )}
                </div>
                <button
                  className="nc-auth-submit"
                  disabled={(!username.trim() || !password) && !working}
                  onClick={submit}
                >
                  {working ? "Signing in…" : "Sign in"}
                </button>
              </>
            ) : step === "key" ? (
              <>
                <div className="nc-field">
                  <label>Access key</label>
                  <div className={`nc-field-box ${error ? "invalid" : ""}`}>
                    <input
                      className="mono"
                      value={accessKey}
                      placeholder="NC-XXXX-XXXX-XXXX-XXXX"
                      autoFocus
                      onChange={(e) => setAccessKey(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && verifyKey()}
                    />
                  </div>
                </div>
                <div className="nc-auth-msg">
                  {error ? (
                    <span className="nc-auth-err">
                      <AlertCircle size={12} />
                      <span>{error}</span>
                    </span>
                  ) : showServer ? (
                    <input
                      className="nc-server-inline"
                      value={serverUrl}
                      onChange={(e) => setServerUrl(e.target.value)}
                      onBlur={() => api.syncSetServerUrl(serverUrl)}
                      placeholder="https://api.notchcat.cloud"
                    />
                  ) : (
                    <span style={{ opacity: 0.6 }}>
                      Contact owner for access key ·{" "}
                      <button
                        type="button"
                        className="nc-linklike"
                        onClick={() => setShowServer(true)}
                      >
                        server
                      </button>
                    </span>
                  )}
                </div>
                <button
                  className="nc-auth-submit"
                  disabled={!accessKey.trim() && !working}
                  onClick={verifyKey}
                >
                  {working ? "Checking…" : "Continue"}
                </button>
              </>
            ) : (
              <>
                <div className="nc-key-ok-row">
                  <CheckCircle2 size={14} style={{ color: "var(--ok-ink)" }} />
                  <span>Access key accepted</span>
                  <span style={{ flex: 1 }} />
                  <button
                    type="button"
                    className="nc-linklike"
                    onClick={() => {
                      setStep("key");
                      setError(null);
                    }}
                  >
                    Change
                  </button>
                </div>
                <div className="nc-field">
                  <label>Username</label>
                  <div className={`nc-field-box ${error ? "invalid" : ""}`}>
                    <input
                      value={username}
                      placeholder="Choose a username"
                      autoFocus
                      onChange={(e) => setUsername(e.target.value)}
                    />
                  </div>
                </div>
                <div className="nc-field">
                  <label>Password</label>
                  <div className="nc-field-box">
                    <input
                      type={showPassword ? "text" : "password"}
                      value={password}
                      placeholder="Password"
                      onChange={(e) => setPassword(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && submit()}
                    />
                    <button
                      type="button"
                      className="nc-eye-btn"
                      onClick={() => setShowPassword((v) => !v)}
                      aria-label={
                        showPassword ? "Hide password" : "Show password"
                      }
                    >
                      {showPassword ? <EyeOff size={13} /> : <Eye size={13} />}
                    </button>
                  </div>
                </div>
                <div className="nc-auth-msg">
                  {error ? (
                    <span className="nc-auth-err">
                      <AlertCircle size={12} />
                      <span>{error}</span>
                    </span>
                  ) : (
                    <span style={{ opacity: 0.6 }}>At least 8 characters</span>
                  )}
                </div>
                <button
                  className="nc-auth-submit"
                  disabled={
                    (!username.trim() || password.length < 8) && !working
                  }
                  onClick={submit}
                >
                  {working ? "Creating…" : "Create account"}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  );
};

// ---------------------------------------------------------------------------
// Main SettingsView
// ---------------------------------------------------------------------------

export const SettingsView: React.FC<{ snapshot: AppSnapshot }> = ({
  snapshot,
}) => {
  const [flow, setFlow] = useState<AuthFlow>(null);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const { settings, sync } = snapshot;

  const patch = (next: Partial<SettingsData>) =>
    api.settingsUpdate({ ...settings, ...next });

  const enabledCount = settings.cards.filter((c) => c.enabled).length;

  if (flow !== null) {
    return (
      <AuthTakeover
        snapshot={snapshot}
        flow={flow}
        grain={settings.paperGrain}
        onFlowChange={setFlow}
      />
    );
  }

  return (
    <div className="nc-settings-scroll">
      <div className="nc-settings-cols">
        <div className="nc-settings-col">
          <Section
            title="Cloud sync"
            footer={
              sync.phase.kind !== "signed_out" && sync.username
                ? null
                : "Contact owner for access key"
            }
          >
            <CloudSyncPanel snapshot={snapshot} onStartFlow={setFlow} />
          </Section>

          <Section title="Tabs">
            <ToggleRow
              title="Week"
              icon={<CalendarDays size={12} />}
              checked={settings.showWeekTab}
              onChange={(v) => patch({ showWeekTab: v })}
            />
            <ToggleRow
              title="Insights"
              icon={<ChartBar size={12} />}
              checked={settings.showInsightsTab}
              onChange={(v) => patch({ showInsightsTab: v })}
            />
          </Section>

          <Section title="Opening">
            <ToggleRow
              title="Open when I hover the notch"
              icon={<MousePointer size={12} />}
              checked={settings.hoverToOpen}
              onChange={(v) => patch({ hoverToOpen: v })}
            />
            <ToggleRow
              title="Open when I click the notch"
              icon={<MousePointer size={12} />}
              checked={settings.clickToOpen}
              onChange={(v) => patch({ clickToOpen: v })}
            />
            <ToggleRow
              title="Ctrl+Space shortcut"
              icon={<Command size={12} />}
              checked={settings.hotkeyEnabled}
              onChange={(v) => patch({ hotkeyEnabled: v })}
            />
          </Section>

          <Section title="Appearance">
            <ToggleRow
              title="Paper grain on cards"
              icon={<Grid3x3 size={12} />}
              checked={settings.paperGrain}
              onChange={(v) => patch({ paperGrain: v })}
            />
            <ToggleRow
              title="Battery percentage in the tray"
              icon={<BatteryMedium size={12} />}
              checked={settings.showBattery}
              onChange={(v) => patch({ showBattery: v })}
            />
            <div className="nc-setting-row">
              <span className="nc-setting-ico">
                <PanelTop size={12} />
              </span>
              <span className="nc-setting-lbl">Fake notch</span>
              <div className="nc-set-seg">
                {(["off", "notch", "island"] as FakeNotchStyle[]).map((st) => (
                  <button
                    key={st}
                    className={settings.fakeNotch === st ? "active" : ""}
                    onClick={() => patch({ fakeNotch: st })}
                  >
                    {st === "off" ? "Off" : st === "notch" ? "Notch" : "Island"}
                  </button>
                ))}
              </div>
            </div>
          </Section>

          <Section title="General">
            <ToggleRow
              title="Open at login"
              icon={<Power size={12} />}
              checked={snapshot.launchAtLogin}
              onChange={(v) => api.settingsSetLaunchAtLogin(v)}
            />
            <ToggleRow
              title="Tell me about new versions"
              icon={<ArrowDownCircle size={12} />}
              checked={settings.checkForUpdates}
              onChange={(v) => {
                patch({ checkForUpdates: v });
                if (v) api.updateCheck();
              }}
            />
            <ActionRow
              title="Back up database"
              icon={<HardDrive size={12} />}
              onClick={async () => {
                try {
                  await api.backupDatabase();
                  setBackupMsg("Saved to Downloads");
                } catch {
                  setBackupMsg("Backup failed");
                }
              }}
            />
            {backupMsg && <div className="nc-setting-note">{backupMsg}</div>}
            <ActionRow
              title="Show data folder"
              icon={<Folder size={12} />}
              onClick={() => api.revealDataFolder()}
            />
            <ActionRow
              title="Quit NotchCat"
              icon={<XCircle size={12} />}
              onClick={() => api.quitApp()}
            />
          </Section>
        </div>

        <div className="nc-settings-col">
          <Section title="Focus timer">
            <ToggleRow
              title="Show countdown beside the notch"
              icon={<RectangleHorizontal size={12} />}
              checked={settings.showPill}
              onChange={(v) => patch({ showPill: v })}
            />
            <StepperRow
              title="Default length"
              icon={<Clock size={12} />}
              value={settings.defaultFocusMinutes}
              min={1}
              max={240}
              step={5}
              unit="min"
              onChange={(v) => patch({ defaultFocusMinutes: v })}
            />
            <ToggleRow
              title="Play sounds (timer and reminders)"
              icon={<Volume2 size={12} />}
              checked={settings.timerSound}
              onChange={(v) => patch({ timerSound: v })}
            />
            <ToggleRow
              title={'Drop a "Time\'s up" card from the notch'}
              icon={<PanelTop size={12} />}
              checked={settings.timeUpIsland}
              onChange={(v) => patch({ timeUpIsland: v })}
            />
          </Section>

          <Section title="Reminders">
            <ToggleRow
              title="Remind me from the notch"
              icon={<Bell size={12} />}
              checked={settings.remindersEnabled}
              onChange={(v) => patch({ remindersEnabled: v })}
            />
            <ToggleRow
              title="Show reminders in Events"
              icon={<List size={12} />}
              checked={settings.showRemindersInEvents}
              onChange={(v) => patch({ showRemindersInEvents: v })}
            />
            <StepperRow
              title="Snooze for"
              icon={<Clock size={12} />}
              value={settings.snoozeMinutes}
              min={1}
              max={60}
              step={1}
              unit="min"
              onChange={(v) => patch({ snoozeMinutes: v })}
            />
          </Section>

          <Section title="Music" footer="Turn on the Now playing card below.">
            <ToggleRow
              title="Album art beside the notch while playing"
              icon={<Music2 size={12} />}
              checked={settings.showMusicChip}
              onChange={(v) => patch({ showMusicChip: v })}
            />
          </Section>

          <Section title="Workspace cards">
            {settings.cards.map((item, idx) => {
              const meta = CARD_META[item.card];
              return (
                <div key={item.card} className="nc-setting-row">
                  <span className="nc-setting-ico">{meta.icon}</span>
                  <span className="nc-setting-lbl">{meta.title}</span>
                  <button
                    className="nc-set-arrow"
                    disabled={idx === 0}
                    onClick={() => api.settingsMoveCard(item.card, -1)}
                  >
                    <ChevronUp size={10} strokeWidth={3} />
                  </button>
                  <button
                    className="nc-set-arrow"
                    disabled={idx === settings.cards.length - 1}
                    onClick={() => api.settingsMoveCard(item.card, 1)}
                  >
                    <ChevronDown size={10} strokeWidth={3} />
                  </button>
                  <AmberSwitch
                    checked={item.enabled}
                    disabled={item.enabled && enabledCount <= 1}
                    onChange={(v) => api.settingsSetCardEnabled(item.card, v)}
                  />
                </div>
              );
            })}
          </Section>
        </div>
      </div>
    </div>
  );
};
