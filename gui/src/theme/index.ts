// Carga del tema embebido (dark/light según config.toml) + inyección
// opcional de theme_path como override — ver docs/fase5-gui.md, "Theming",
// y THEME.md. Fallback silencioso si el override no existe/es inválido.
import { invoke } from '@tauri-apps/api/core';
import darkCss from './dark.css?raw';
import lightCss from './light.css?raw';

export interface ThemeConfig {
  theme: string;
  theme_path: string;
}

/** Acento por defecto de cada tema embebido (ver dark.css/light.css) — usado
 *  por `views/Settings.ts` para el botón "Restablecer". */
export const DEFAULT_ACCENT: Record<'dark' | 'light', string> = {
  dark: '#7c6fff',
  light: '#6952e0',
};

/** Reutilizado por `views/Settings.ts` para leer el estado actual sin
 *  duplicar el manejo de errores de `read_theme_config`. */
export async function getThemeConfig(): Promise<ThemeConfig> {
  try {
    return await invoke<ThemeConfig>('read_theme_config');
  } catch {
    return { theme: 'dark', theme_path: '' };
  }
}

/** Reutilizado por `views/Settings.ts` para la vista previa en vivo del acento. */
export function injectStyle(id: string, css: string) {
  let el = document.getElementById(id) as HTMLStyleElement | null;
  if (!el) {
    el = document.createElement('style');
    el.id = id;
    document.head.appendChild(el);
  }
  el.textContent = css;
}

export async function loadTheme(): Promise<void> {
  const config = await getThemeConfig();

  const base = config.theme === 'light' ? lightCss : darkCss;
  injectStyle('photoranker-theme-base', base);
  document.documentElement.dataset.theme = config.theme === 'light' ? 'light' : 'dark';

  if (config.theme_path && config.theme_path.trim() !== '') {
    try {
      const override = await invoke<string | null>('read_theme_override', {
        path: config.theme_path,
      });
      if (override) {
        injectStyle('photoranker-theme-override', override);
      }
    } catch {
      // Silencioso a propósito — un CSS de usuario mal formado no debe romper la app.
    }
  }
}
