import React, { useEffect, useState } from "react";
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
  const isOpen = snapshot?.ui.isOpen ?? false;
  const [trayMounted, setTrayMounted] = useState(isOpen);
  const [trayExpanded, setTrayExpanded] = useState(isOpen);

  useEffect(() => {
    if (isOpen) {
      setTrayMounted(true);
      let raf2 = 0;
      const raf1 = requestAnimationFrame(() => {
        raf2 = requestAnimationFrame(() => {
          setTrayExpanded(true);
        });
      });
      return () => {
        cancelAnimationFrame(raf1);
        if (raf2) cancelAnimationFrame(raf2);
      };
    }
    setTrayExpanded(false);
    const timer = window.setTimeout(() => setTrayMounted(false), 250);
    return () => window.clearTimeout(timer);
  }, [isOpen]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && isOpen) {
        // Modals/inputs with custom Escape handlers call stopPropagation();
        // reaching here means closing the main tray itself.
        api.closeTray();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen]);

  if (!snapshot) {
    return null;
  }

  const { ui } = snapshot;

  const handleShellPointerDown = () => {
    if (ui.isOpen && !ui.pinned) {
      api.openTray(true);
    }
  };

  return (
    <div className="nc-stage">
      {!trayMounted ? (
        <CollapsedIsland snapshot={snapshot} />
      ) : (
        <main
          className={`nc-tray ${trayExpanded ? "is-expanded" : "is-collapsed"} ${
            snapshot.settings.fakeNotch === "island" ? "from-island" : "from-notch"
          }`}
          style={
            {
              "--panel-w": `${ui.panelW}px`,
              "--panel-h": `${ui.panelH}px`,
            } as React.CSSProperties
          }
          onPointerDown={handleShellPointerDown}
        >
          <div
            className="nc-tray-inner"
            style={{ width: ui.panelW, height: ui.panelH }}
          >
            <HeaderBar snapshot={snapshot} />
            <div className="nc-tray-content">
              {ui.tab === "workspace" && <WorkspaceView snapshot={snapshot} />}
              {ui.tab === "week" && <WeekView snapshot={snapshot} />}
              {ui.tab === "insights" && <InsightsView snapshot={snapshot} />}
              {ui.tab === "settings" && <SettingsView snapshot={snapshot} />}
            </div>
          </div>
        </main>
      )}
    </div>
  );
};
