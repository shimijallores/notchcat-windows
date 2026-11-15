import React, { useEffect } from "react";
import { api } from "./api";
import { CollapsedIsland } from "./components/CollapsedIsland";
import { HeaderBar } from "./components/HeaderBar";
import { InsightsView } from "./components/InsightsView";
import { SettingsView } from "./components/SettingsView";
import { WeekView } from "./components/WeekView";
import { WorkspaceView } from "./components/WorkspaceView";
import { useNotchCat } from "./useNotchCat";

export const App: React.FC = () => {
  const snapshot = useNotchCat();

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && snapshot?.ui.isOpen) {
        // Modals/inputs with custom Escape handlers call stopPropagation();
        // reaching document means closing the main tray itself.
        api.closeTray();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [snapshot?.ui.isOpen]);

  if (!snapshot) {
    return null;
  }

  const { ui, settings } = snapshot;
  const isIsland = settings.fakeNotch === "island";

  const handleShellPointerDown = () => {
    if (ui.isOpen && !ui.pinned) {
      api.openTray(true);
    }
  };

  return (
    <div className="nc-stage">
      {!ui.isOpen ? (
        <CollapsedIsland snapshot={snapshot} />
      ) : (
        <main
          className={`nc-tray ${isIsland ? "island-mode" : "notch-mode"}`}
          style={{
            width: ui.panelW,
            height: ui.panelH,
          }}
          onPointerDown={handleShellPointerDown}
        >
          <HeaderBar snapshot={snapshot} />
          <div className="nc-tray-content">
            {ui.tab === "workspace" && <WorkspaceView snapshot={snapshot} />}
            {ui.tab === "week" && <WeekView snapshot={snapshot} />}
            {ui.tab === "insights" && <InsightsView snapshot={snapshot} />}
            {ui.tab === "settings" && <SettingsView snapshot={snapshot} />}
          </div>
        </main>
      )}
    </div>
  );
};
