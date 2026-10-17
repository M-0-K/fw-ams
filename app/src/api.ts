import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Snapshot } from "./bindings/Snapshot";
import type { Settings } from "./bindings/Settings";
import type { ChargeControl } from "./bindings/ChargeControl";
import type { HistorySample } from "./bindings/HistorySample";
import type { PowerMode } from "./bindings/PowerMode";
import type { TunerConfig } from "./bindings/TunerConfig";
import type { TunerState } from "./bindings/TunerState";
import type { TunerView } from "./bindings/TunerView";

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

// --- Advanced: CPU tuner, history, charge control -------------------------------------------

/** Same cap as the backend history (10 min at 1 Hz) */
const HISTORY = 600;

export const tunerSetConfig = (config: TunerConfig) => invoke<TunerState>("tuner_set_config", { config });
export const tunerReset = () => invoke<TunerState>("tuner_reset");
export const tunerInstall = () => invoke<void>("tuner_install");
export const tunerUninstall = () => invoke<void>("tuner_uninstall");
export const setPowerMode = (mode: PowerMode) => invoke<void>("set_power_mode", { mode });
export const openPawnioSite = () => invoke<void>("open_pawnio_site");
export const chargeFullOnce = () => invoke<ChargeControl>("charge_full_once");

/** Tuner state and live history, pushed once per second by the backend */
export function useTuner(): { view: TunerView | null; history: HistorySample[] } {
  const [view, setView] = useState<TunerView | null>(null);
  const [history, setHistory] = useState<HistorySample[]>([]);

  useEffect(() => {
    let active = true;
    invoke<TunerView | null>("get_tuner").then((v) => active && v && setView(v));
    invoke<HistorySample[]>("get_history").then((h) => active && setHistory(h));
    const unlisten = listen<{ view: TunerView; sample: HistorySample }>("tuner", (e) => {
      if (!active) return;
      setView(e.payload.view);
      setHistory((h) => [...h.slice(-(HISTORY - 1)), e.payload.sample]);
    });
    return () => {
      active = false;
      unlisten.then((f) => f());
    };
  }, []);

  return { view, history };
}

export function useChargeControl(): ChargeControl | null {
  const [charge, setCharge] = useState<ChargeControl | null>(null);

  useEffect(() => {
    let active = true;
    invoke<ChargeControl>("get_charge_control").then((c) => active && setCharge(c));
    const unlisten = listen<ChargeControl>("charge", (e) => active && setCharge(e.payload));
    return () => {
      active = false;
      unlisten.then((f) => f());
    };
  }, []);

  return charge;
}
