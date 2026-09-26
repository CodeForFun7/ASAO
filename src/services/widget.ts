import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AsaoSettings,
  WidgetMode,
  WidgetSystemUpdate,
} from "../types/widget";

export const WIDGET_EVENTS = {
  SYSTEM_UPDATE: "system:update",
  SETTINGS_UPDATED: "settings:updated",
  NAVIGATE_INSPECT: "asao:navigate-inspect",
  NAVIGATE_ROUTE: "asao:navigate",
} as const;

export async function getWidgetUpdate(): Promise<WidgetSystemUpdate> {
  return invoke<WidgetSystemUpdate>("get_widget_update");
}

export async function getSettings(): Promise<AsaoSettings> {
  return invoke<AsaoSettings>("get_settings");
}

export async function updateSettings(
  newSettings: AsaoSettings
): Promise<AsaoSettings> {
  return invoke<AsaoSettings>("update_settings", { newSettings });
}

export async function showWidget(): Promise<boolean> {
  return invoke<boolean>("show_widget");
}

export async function hideWidget(): Promise<void> {
  return invoke<void>("hide_widget");
}

export async function setWidgetMode(mode: WidgetMode): Promise<void> {
  return invoke<void>("set_widget_mode", { mode });
}

export async function openMainWindow(
  route?: string,
  inspectPid?: number
): Promise<void> {
  return invoke<void>("open_main_window", {
    route: route ?? null,
    inspectPid: inspectPid ?? null,
  });
}

export async function subscribeToWidgetEvents(handlers: {
  onSystemUpdate?: (update: WidgetSystemUpdate) => void;
  onSettingsUpdated?: (settings: AsaoSettings) => void;
}): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];

  if (handlers.onSystemUpdate) {
    const unlistenUpdate = await listen<WidgetSystemUpdate>(
      WIDGET_EVENTS.SYSTEM_UPDATE,
      (event) => {
        handlers.onSystemUpdate?.(event.payload);
      }
    );
    unlisteners.push(unlistenUpdate);
  }

  if (handlers.onSettingsUpdated) {
    const unlistenSettings = await listen<AsaoSettings>(
      WIDGET_EVENTS.SETTINGS_UPDATED,
      (event) => {
        handlers.onSettingsUpdated?.(event.payload);
      }
    );
    unlisteners.push(unlistenSettings);
  }

  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
