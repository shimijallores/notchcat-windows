import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import type { AppSnapshot } from "./types";

export function useNotchCat(): AppSnapshot | null {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);

  useEffect(() => {
    let active = true;
    api.getSnapshot().then((s) => {
      if (active) setSnapshot(s);
    });

    const unlistenPromise = listen<AppSnapshot>("app-state-changed", (ev) => {
      if (active) setSnapshot(ev.payload);
    });

    return () => {
      active = false;
      unlistenPromise.then((u) => u());
    };
  }, []);

  return snapshot;
}

export function formatClock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
}

export function formatDurationShort(seconds: number): string {
  const mins = Math.round(seconds / 60);
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h > 0) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  return `${m}m`;
}
