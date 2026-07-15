// Carga del tema embebido (dark/light según config.toml) + inyección
// opcional de theme_path como override — ver docs/fase5-gui.md, "Theming",
// y THEME.md. Fallback silencioso si el override no existe/es inválido.
import { invoke } from '@tauri-apps/api/core';
import darkCss from './dark.css?raw';
import lightCss from './light.css?raw';

interface ThemeConfig {
  theme: string;
  theme_path: string;
}

function injectStyle(id: string, css: string) {
  let el = document.getElementById(id) as HTMLStyleElement | null;
  if (!el) {
    el = document.createElement('style');
    el.id = id;
    document.head.appendChild(el);
  }
  el.textContent = css;
}

export async function loadTheme(): Promise<void> {
  let config: ThemeConfig = { theme: 'dark', theme_path: '' };
  try {
    config = await invoke<ThemeConfig>('read_theme_config');
  } catch {
    // config.toml aún no existe (ningún comando del CLI corrió todavía) —
    // se usa el default embebido, no es un error bloqueante.
  }

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
