export type CardID = "tasks" | "timer" | "notepad" | "events" | "music";
export type FakeNotchStyle = "off" | "notch" | "island";
export type TabID = "workspace" | "week" | "insights" | "settings";
export type TimerState = "idle" | "running" | "paused" | "finished";
export type MusicPlayer = "spotify" | "appleMusic" | "systemMedia";

export interface TaskItem {
  id: string;
  title: string;
  isDone: boolean;
  day: number;
  plannedDay: number;
  timeLimitMinutes: number | null;
  remindAt: number | null;
  reminded: boolean;
  createdAt: number;
  completedAt: number | null;
}

export interface FinishedTask {
  id: string;
  title: string;
  completedAt: number;
  plannedDay: number;
  timeLimitMinutes: number | null;
  focusSeconds: number;
}

export interface CardSetting {
  card: CardID;
  enabled: boolean;
}

export interface SettingsData {
  cards: CardSetting[];
  showWeekTab: boolean;
  showInsightsTab: boolean;
  showPill: boolean;
  defaultFocusMinutes: number;
  timerSound: boolean;
  timeUpIsland: boolean;
  remindersEnabled: boolean;
  snoozeMinutes: number;
  showRemindersInEvents: boolean;
  icsUrl: string | null;
  hoverToOpen: boolean;
  clickToOpen: boolean;
  hotkeyEnabled: boolean;
  showMusicChip: boolean;
  paperGrain: boolean;
  fakeNotch: FakeNotchStyle;
  showBattery: boolean;
  checkForUpdates: boolean;
}

export interface TimerSnapshot {
  state: TimerState;
  remaining: number;
  sessionTotal: number;
  linkedTaskId: string | null;
  progress: number;
}

export interface BatteryStatus {
  percent: number | null;
  charging: boolean;
}

export interface NowPlayingTrack {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: number;
  elapsed: number;
  updatedAtMs: number;
  playing: boolean;
  player: MusicPlayer;
  artworkDataUrl: string | null;
  pastelTint: [number, number, number] | null;
}

export interface MusicSnapshot {
  track: NowPlayingTrack | null;
  effectivePlayingSource: MusicPlayer | null;
  hasSession: boolean;
}

export type CalendarAccess =
  | { kind: "notDetermined" }
  | { kind: "authorized" }
  | { kind: "failed"; message: string };

export interface CalendarEvent {
  id: string;
  title: string;
  startMs: number;
  endMs: number;
  isAllDay: boolean;
  color: [number, number, number];
}

export interface CalendarSnapshot {
  access: CalendarAccess;
  events: CalendarEvent[];
}

export type SyncPhase =
  | { kind: "signed_out" }
  | { kind: "idle" }
  | { kind: "syncing" }
  | { kind: "offline" }
  | { kind: "failed"; message: string }
  | { kind: "revoked" };

export interface SyncStatusSnapshot {
  phase: SyncPhase;
  lastSyncAt: number | null;
  pendingCount: number;
  serverUrl: string;
  username: string | null;
}

export interface ReleaseInfo {
  version: string;
  url: string;
}

export interface InsightsDay {
  date: number;
  planned: number;
  completed: number;
  focusSeconds: number;
}

export interface InsightsSummary {
  days: InsightsDay[];
  completed: number;
  planned: number;
  focusSeconds: number;
  activeDays: number;
  previousCompleted: number;
  headline: string;
  sentence: string;
}

export interface HistorySection {
  day: number;
  title: string;
  items: FinishedTask[];
  focusSeconds: number;
}

export interface HistoryPage {
  sections: HistorySection[];
  total: number;
  canLoadMore: boolean;
}

export interface ReminderPresetOption {
  id: string;
  title: string;
  timestamp: number;
}

export interface WeekDaySnapshot {
  day: number;
  tasks: TaskItem[];
}

export type CompactBanner =
  | { kind: "timesUp"; sound: boolean; taskTitle: string | null }
  | { kind: "reminder"; taskId: string; title: string };

export interface UiSnapshot {
  isOpen: boolean;
  pinned: boolean;
  tab: TabID;
  insightsWeekOffset: number;
  compact: CompactBanner | null;
  pendingReminder: TaskItem | null;
  overlayW: number;
  panelW: number;
  panelH: number;
}

export interface AppSnapshot {
  version: string;
  ui: UiSnapshot;
  todayStartSecs: number;
  todayTasks: TaskItem[];
  tomorrowTasks: TaskItem[];
  weekDays: WeekDaySnapshot[];
  todayReminders: TaskItem[];
  reminderPresets: ReminderPresetOption[];
  noteText: string;
  noteWordCount: number;
  noteHasUndo: boolean;
  timer: TimerSnapshot;
  settings: SettingsData;
  launchAtLogin: boolean;
  battery: BatteryStatus;
  music: MusicSnapshot;
  calendar: CalendarSnapshot;
  sync: SyncStatusSnapshot;
  updateAvailable: ReleaseInfo | null;
  updateStatusMessage: string | null;
  insights: InsightsSummary;
}

export interface ConvertLineResult {
  converted: boolean;
  nextText: string;
  cursorUtf16: number;
}
