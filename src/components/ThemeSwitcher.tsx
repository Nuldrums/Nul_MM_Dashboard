import { useThemeContext } from '../theme/ThemeProvider';
import { Check } from 'lucide-react';

export default function ThemeSwitcher() {
  const { themeId, themes, setTheme } = useThemeContext();

  return (
    <div className="theme-grid">
      {themes.map((t) => {
        const isSelected = t.id === themeId;
        const colors = t.colors as Record<string, string>;
        return (
          <div
            key={t.id}
            className={`theme-swatch${isSelected ? ' selected' : ''}`}
            onClick={() => setTheme(t.id)}
          >
            <div className="theme-swatch-colors">
              <span style={{ backgroundColor: colors.accent_primary }} />
              <span style={{ backgroundColor: colors.bg_primary }} />
              <span style={{ backgroundColor: colors.surface_sidebar }} />
              <span style={{ backgroundColor: colors.chart_2 }} />
              <span style={{ backgroundColor: colors.success }} />
            </div>
            <div className="flex-between">
              <div>
                <div className="theme-swatch-name">{t.name}</div>
                <div className="theme-swatch-desc">{t.description}</div>
              </div>
              {isSelected && (
                <Check
                  size={16}
                  style={{ color: 'var(--accent-primary)' }}
                />
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
