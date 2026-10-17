import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Snapshot } from "./bindings/Snapshot";
import type { Settings } from "./bindings/Settings";

export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<Settings>("set_settings", { settings });

/** Latest snapshot pushed by the backend poller */
export function useSnapshot(): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  useEffect(() => {
    let active = true;
    invoke<Snapshot | null>("get_snapshot").then((s) => active && s && setSnapshot(s));
    const unlisten = listen<Snapshot>("snapshot", (e) => active && setSnapshot(e.payload));
    return () => {
      active = false;
      unlisten.then((f) => f());
    };
  }, []);

  return snapshot;
}

/** Current settings, kept live through the backend's "settings" event */
export function useSettings(): Settings | null {
  const [settings, setLocal] = useState<Settings | null>(null);

  useEffect(() => {
    let active = true;
    getSettings().then((s) => active && setLocal(s)).catch(() => {});
    const unlisten = listen<Settings>("settings", (e) => active && setLocal(e.payload));
    return () => {
      active = false;
      unlisten.then((f) => f());
    };
  }, []);

  return settings;
}
