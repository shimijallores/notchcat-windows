import { invoke } from "@tauri-apps/api/core";
import type {
  AppSnapshot,
  CardID,
  ConvertLineResult,
  HistoryPage,
  SettingsData,
  TabID,
} from "./types";

export const api = {
  getSnapshot: () => invoke<AppSnapshot>("get_snapshot"),
  openTray: (pin: boolean) => invoke<void>("open_tray", { pin }),
  closeTray: () => invoke<void>("close_tray"),
  setTab: (tab: TabID) => invoke<void>("set_tab", { tab }),
  setInsightsWeekOffset: (offset: number) =>
    invoke<void>("set_insights_week_offset", { offset }),
  setModalOpen: (open: boolean) => invoke<void>("set_modal_open", { open }),
  setDragActive: (active: boolean) => invoke<void>("set_drag_active", { active }),
  dismissCompact: () => invoke<void>("dismiss_compact"),

  // Tasks
  addTask: (title: string, day?: number, timeLimitMinutes?: number | null) =>
    invoke<void>("add_task", {
      title,
      day: day ?? null,
      timeLimitMinutes: timeLimitMinutes ?? null,
    }),
  toggleTask: (id: string) => invoke<void>("toggle_task", { id }),
  renameTask: (id: string, title: string) =>
    invoke<void>("rename_task", { id, title }),
  setTaskTimeLimit: (id: string, minutes: number | null) =>
    invoke<void>("set_task_time_limit", { id, minutes }),
  moveTaskToDate: (id: string, day: number) =>
    invoke<void>("move_task_to_date", { id, day }),
  duplicateTask: (id: string) => invoke<void>("duplicate_task", { id }),
  deleteTask: (id: string) => invoke<void>("delete_task", { id }),
  reorderTasks: (day: number, orderedIds: string[]) =>
    invoke<void>("reorder_tasks", { day, orderedIds }),
  setTaskReminder: (id: string, remindAt: number | null) =>
    invoke<void>("set_task_reminder", { id, remindAt }),
  restoreTask: (id: string) => invoke<void>("restore_task", { id }),

  // Reminders
  reminderComplete: (taskId: string) =>
    invoke<void>("reminder_complete", { taskId }),
  reminderSnooze: (taskId: string) =>
    invoke<void>("reminder_snooze", { taskId }),
  reminderDismiss: () => invoke<void>("reminder_dismiss"),

  // Focus timer
  timerStart: (minutes?: number | null, taskId?: string | null) =>
    invoke<void>("timer_start", {
      minutes: minutes ?? null,
      taskId: taskId ?? null,
    }),
  timerToggle: () => invoke<void>("timer_toggle"),
  timerReset: () => invoke<void>("timer_reset"),
  timerSetDuration: (minutes: number) =>
    invoke<void>("timer_set_duration", { minutes }),
  timerAddTime: (seconds: number) => invoke<void>("timer_add_time", { seconds }),

  // Notepad
  noteSetText: (text: string) => invoke<void>("note_set_text", { text }),
  noteConvertLine: (cursorUtf16: number) =>
    invoke<ConvertLineResult>("note_convert_line", { cursorUtf16 }),
  noteUndo: () => invoke<void>("note_undo"),

  // History
  getHistory: (search: string, pageSize: number) =>
    invoke<HistoryPage>("get_history", { search, pageSize }),

  // Settings
  settingsUpdate: (data: SettingsData) =>
    invoke<void>("settings_update", { data }),
  settingsSetCardEnabled: (card: CardID, enabled: boolean) =>
    invoke<void>("settings_set_card_enabled", { card, enabled }),
  settingsMoveCard: (card: CardID, offset: number) =>
    invoke<void>("settings_move_card", { card, offset }),
  settingsSetLaunchAtLogin: (enabled: boolean) =>
    invoke<void>("settings_set_launch_at_login", { enabled }),

  // Music
  musicPlayPause: () => invoke<void>("music_play_pause"),
  musicNext: () => invoke<void>("music_next"),
  musicPrevious: () => invoke<void>("music_previous"),
  musicLaunchPlayer: () => invoke<void>("music_launch_player"),

  // Calendar & Updates
  calendarRefresh: () => invoke<void>("calendar_refresh"),
  updateCheck: () => invoke<void>("update_check"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  revealDataFolder: () => invoke<void>("reveal_data_folder"),
  backupDatabase: (destPath?: string) =>
    invoke<string>("backup_database", { destPath: destPath ?? null }),

  // Sync
  syncCheckKey: (serverUrl: string, key: string) =>
    invoke<void>("sync_check_key", { serverUrl, key }),
  syncSignUp: (
    serverUrl: string,
    accessKey: string,
    username: string,
    password: string,
  ) =>
    invoke<void>("sync_sign_up", { serverUrl, accessKey, username, password }),
  syncSignIn: (serverUrl: string, username: string, password: string) =>
    invoke<void>("sync_sign_in", { serverUrl, username, password }),
  syncSignOut: () => invoke<void>("sync_sign_out"),
  syncNow: () => invoke<void>("sync_now"),
  syncSetServerUrl: (serverUrl: string) =>
    invoke<void>("sync_set_server_url", { serverUrl }),

  quitApp: () => invoke<void>("quit_app"),
};
