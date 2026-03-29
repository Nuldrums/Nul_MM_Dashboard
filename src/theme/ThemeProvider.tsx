import { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import themes from './themes.json';

interface ThemeColors {
  [key: string]: string;
}

interface Theme {
  id: string;
  name: string;
  description: string;
  colors: ThemeColors;
}

interface ThemeContextValue {
  themeId: string;
  theme: Theme;
  themes: Theme[];
  setTheme: (id: string) => void;
}

const STORAGE_KEY = 'trikeri-theme';
const DEFAULT_THEME = 'peach_sunset';

const ThemeContext = createContext<ThemeContextValue | null>(null);

function applyThemeToDOM(theme: Theme) {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(theme.colors)) {
    root.style.setProperty(`--${key.replace(/_/g, '-')}`, value);
  }
}

function getInitialThemeId(): string {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored && themes.find((t) => t.id === stored)) {
      return stored;
    }
  } catch {
    // localStorage not available
  }
  return DEFAULT_THEME;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeId, setThemeId] = useState<string>(getInitialThemeId);

  const theme = themes.find((t) => t.id === themeId) ?? themes[0];

  useEffect(() => {
    applyThemeToDOM(theme);
    try {
      localStorage.setItem(STORAGE_KEY, themeId);
    } catch {
      // localStorage not available
    }
  }, [themeId, theme]);

  const setTheme = (id: string) => {
    if (themes.find((t) => t.id === id)) {
      setThemeId(id);
    }
  };

  return (
    <ThemeContext.Provider value={{ themeId, theme, themes, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useThemeContext() {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error('useThemeContext must be used within a ThemeProvider');
  }
  return ctx;
}
