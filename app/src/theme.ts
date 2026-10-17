import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import type { Theme } from "./bindings/Theme";

const KEY = "fw-ams-theme";
const query = () => window.matchMedia("(prefers-color-scheme: dark)");

const isDark = (theme: Theme) => (theme === "system" ? query().matches : theme === "dark");

function apply(theme: Theme) {
  document.documentElement.classList.toggle("dark", isDark(theme));
  // null = follow the OS for the native title bar
  getCurrentWindow()
    .setTheme(theme === "system" ? null : theme)
    .catch(() => {});
}

/** Set the class from the cached value before React renders, so there is no flash */
export function applyCachedTheme() {
  let theme: Theme = "system";
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "system") theme = v;
  } catch {
    // storage unavailable
  }
  document.documentElement.classList.toggle("dark", isDark(theme));
}

/** Keeps <html> and the window title bar in sync with the chosen theme */
export function useTheme(theme: Theme | undefined) {
  useEffect(() => {
    if (!theme) return;
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      // storage unavailable
    }
    apply(theme);
    if (theme !== "system") return;
    const mq = query();
    const onChange = () => apply(theme);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme]);
}
