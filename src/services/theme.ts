export type ThemeMode = "dark" | "light" | "system";

const STORAGE_KEY = "asao-theme";

let mediaQueryListenerAttached = false;
let currentThemeMode: ThemeMode = "dark";

export function getSystemTheme(): "dark" | "light" {
  if (typeof window === "undefined" || !window.matchMedia) {
    return "dark";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function getEffectiveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode === "system") {
    return getSystemTheme();
  }
  return mode;
}

export function getStoredTheme(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
    if (saved === "dark" || saved === "light" || saved === "system") {
      return saved;
    }
  } catch {
    // Ignore localStorage errors
  }
  return "dark";
}

export function applyTheme(mode: ThemeMode) {
  currentThemeMode = mode;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // Ignore localStorage errors
  }

  const effective = getEffectiveTheme(mode);
  const root = document.documentElement;

  if (effective === "light") {
    root.classList.remove("dark");
    root.classList.add("light");
    root.setAttribute("data-theme", "light");
    root.style.colorScheme = "light";
  } else {
    root.classList.remove("light");
    root.classList.add("dark");
    root.setAttribute("data-theme", "dark");
    root.style.colorScheme = "dark";
  }

  setupMediaQueryListener();
}

function setupMediaQueryListener() {
  if (mediaQueryListenerAttached || typeof window === "undefined" || !window.matchMedia) {
    return;
  }

  try {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      if (currentThemeMode === "system") {
        applyTheme("system");
      }
    };

    if (mql.addEventListener) {
      mql.addEventListener("change", handler);
    } else {
      // Fallback for older browsers
      mql.addListener(handler);
    }
    mediaQueryListenerAttached = true;
  } catch {
    // Ignore media query error
  }
}

export function initializeTheme(savedTheme?: ThemeMode) {
  const initial = savedTheme || getStoredTheme();
  applyTheme(initial);
}
