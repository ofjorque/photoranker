// Carga del tema embebido (dark/light según config.toml) + inyección
// opcional de theme_path como override — ver docs/fase5-gui.md, "Theming",
// y THEME.md. Fallback silencioso si el override no existe/es inválido.
import { invoke } from '@tauri-apps/api/core';

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

/** Inyecta el CSS base embebido (dark.css/light.css completo) — la única
 *  fuente de verdad del tema activo, vía variables CSS (ver fase5-gui.md,
 *  "Theming"). Reutilizado por `views/Settings.ts` para que "Guardar"
 *  cambie el tema al instante sin reiniciar la app. */
export function applyBaseTheme(theme: 'dark' | 'light'): void {
  if (theme === 'dark') {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

/** `#rrggbb` → triplete `"H S% L%"` (sin `hsl()`) — el formato que
 *  `index.css`/`tailwind.config.js` esperan en cada variable `--x` (así
 *  Tailwind puede componerlas como `hsl(var(--x))` o `hsl(var(--x) / alpha)`).
 *  Usado por `views/Settings.tsx` para que el acento elegido en el color
 *  picker (`#rrggbb`) controle `--primary`, el token real que leen los
 *  componentes de shadcn — no alcanza con inyectar un token propio como
 *  `--color-accent` que ningún componente lee. */
export function hexToHslTriplet(hex: string): string {
  const n = parseInt(hex.replace('#', ''), 16);
  const r = ((n >> 16) & 255) / 255;
  const g = ((n >> 8) & 255) / 255;
  const b = (n & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  let h = 0;
  const l = (max + min) / 2;
  const d = max - min;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  if (d !== 0) {
    switch (max) {
      case r:
        h = ((g - b) / d) % 6;
        break;
      case g:
        h = (b - r) / d + 2;
        break;
      default:
        h = (r - g) / d + 4;
    }
    h *= 60;
    if (h < 0) h += 360;
  }
  return `${Math.round(h)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%`;
}

export async function loadTheme(): Promise<void> {
  const config = await getThemeConfig();

  applyBaseTheme(config.theme === 'light' ? 'light' : 'dark');

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
