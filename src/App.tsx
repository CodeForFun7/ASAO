import { useEffect, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppShell } from "./components/layout/AppShell";
import { Dashboard } from "./pages/Dashboard";
import { Processes } from "./pages/Processes";
import { AsaoWidget } from "./components/widget/AsaoWidget";
import { WidgetSettings } from "./components/widget/WidgetSettings";
import { useProcessStore } from "./stores/process-store";
import type { AppRoute } from "./types/process";

function isDedicatedWidgetWindow(): boolean {
  if (typeof window !== "undefined") {
    if (window.location.search.includes("window=widget")) {
      return true;
    }
    try {
      if (getCurrentWindow().label === "widget") {
        return true;
      }
    } catch {
      // Fallback in standard browser context
    }
  }
  return false;
}

function App() {
  const isWidgetWindow = useMemo(() => isDedicatedWidgetWindow(), []);
  const currentRoute = useProcessStore((s) => s.currentRoute);
  const setRoute = useProcessStore((s) => s.setRoute);
  const selectProcess = useProcessStore((s) => s.selectProcess);
  const initializeMonitoring = useProcessStore((s) => s.initializeMonitoring);

  // Transparent background override for the frameless widget window
  useEffect(() => {
    if (isWidgetWindow) {
      document.documentElement.classList.add("widget-window");
      document.documentElement.style.background = "transparent";
      document.body.style.background = "transparent";
      const root = document.getElementById("root");
      if (root) root.style.background = "transparent";
      return;
    }

    let cleanupMonitoring: (() => void) | undefined;
    let unlistenNav: (() => void) | undefined;
    let unlistenInspect: (() => void) | undefined;
    let mounted = true;

    void initializeMonitoring().then((unlisten) => {
      if (mounted) {
        cleanupMonitoring = unlisten;
      } else {
        unlisten();
      }
    });

    void listen<string>("asao:navigate", (event) => {
      const target = event.payload as AppRoute;
      if (target === "dashboard" || target === "processes" || target === "settings") {
        setRoute(target);
      }
    }).then((fn) => {
      if (mounted) unlistenNav = fn;
      else fn();
    });

    void listen<{ route: string; pid: number | null }>(
      "asao:navigate-inspect",
      (event) => {
        const { route, pid } = event.payload;
        if (route === "dashboard" || route === "processes" || route === "settings") {
          setRoute(route);
        }
        if (pid !== null && pid !== undefined) {
          selectProcess(pid);
        }
      }
    ).then((fn) => {
      if (mounted) unlistenInspect = fn;
      else fn();
    });

    return () => {
      mounted = false;
      if (cleanupMonitoring) cleanupMonitoring();
      if (unlistenNav) unlistenNav();
      if (unlistenInspect) unlistenInspect();
    };
  }, [isWidgetWindow, initializeMonitoring, setRoute, selectProcess]);

  if (isWidgetWindow) {
    return <AsaoWidget />;
  }

  return (
    <AppShell>
      {currentRoute === "dashboard" && <Dashboard />}
      {currentRoute === "processes" && <Processes />}
      {currentRoute === "settings" && <WidgetSettings />}
    </AppShell>
  );
}

export default App;
