// Ajustes — personalización de color (feedback de uso real: "que el
// usuario pueda cambiar los colores de la GUI"). Construida SOBRE el
// mecanismo de override ya existente (config.toml: theme/theme_path, ver
// docs/fase5-gui.md, "Theming") en vez de reemplazarlo: esta pantalla solo
// genera el CSS de override y lo persiste vía los comandos Tauri
// write_theme_config/write_theme_override.
import { invoke } from '@tauri-apps/api/core';
import { DEFAULT_ACCENT, getThemeConfig, injectStyle } from '../theme';
import { showToast } from '../toast';
import { confirmDialog } from '../components/ConfirmDialog';

const PREVIEW_STYLE_ID = 'photoranker-theme-override';

// `--color-danger` de cada tema embebido (ver dark.css/light.css) — fijo, no
// personalizable en esta pantalla. Se usa para avisar si el acento elegido
// queda demasiado parecido (feedback de uso real: eligiendo un acento rojo,
// los botones "primarios" y los destructivos terminaron siendo casi
// indistinguibles entre sí).
const DANGER_COLOR: Record<'dark' | 'light', string> = {
  dark: '#e55a6a',
  light: '#c73b4c',
};
const SIMILARITY_WARN_DISTANCE = 90; // distancia euclídea en RGB (máx. ~441)

function accentCss(theme: 'dark' | 'light', accent: string): string {
  const softAlpha = theme === 'light' ? '12%' : '18%';
  return `:root {
  --color-accent: ${accent};
  --color-accent-soft: color-mix(in srgb, ${accent} ${softAlpha}, transparent);
}`;
}

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', ''), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function colorDistance(a: string, b: string): number {
  const [r1, g1, b1] = hexToRgb(a);
  const [r2, g2, b2] = hexToRgb(b);
  return Math.sqrt((r1 - r2) ** 2 + (g1 - g2) ** 2 + (b1 - b2) ** 2);
}

export async function renderSettings(container: HTMLElement): Promise<void> {
  const config = await getThemeConfig();
  const initialTheme: 'dark' | 'light' = config.theme === 'light' ? 'light' : 'dark';

  const isGuiManaged = await invoke<boolean>('theme_path_is_gui_managed', {
    themePath: config.theme_path ?? '',
  }).catch(() => true);

  // El acento actual solo se puede leer de forma confiable desde el DOM (la
  // variable ya está aplicada por loadTheme() al arrancar la app) — evita
  // tener que volver a parsear el CSS de override a mano acá.
  const currentAccent =
    getComputedStyle(document.documentElement).getPropertyValue('--color-accent').trim() ||
    DEFAULT_ACCENT[initialTheme];

  container.innerHTML = `
    <div class="view">
      <h1>Ajustes</h1>
      <div class="panel">
        <h2>Apariencia</h2>
        <p>Los cambios se ven al instante; "Guardar" los deja persistidos para la próxima vez que abras la app.</p>

        <div class="field" style="margin-top:16px">
          <label>Tema base</label>
          <div class="input-row">
            <label style="display:flex; align-items:center; gap:6px; font-weight:400; cursor:pointer">
              <input type="radio" name="base-theme" value="dark" ${initialTheme === 'dark' ? 'checked' : ''} />
              Oscuro
            </label>
            <label style="display:flex; align-items:center; gap:6px; font-weight:400; cursor:pointer">
              <input type="radio" name="base-theme" value="light" ${initialTheme === 'light' ? 'checked' : ''} />
              Claro
            </label>
          </div>
        </div>

        <div class="field" style="margin-top:16px">
          <label>Color de acento</label>
          <div class="input-row">
            <input type="color" id="accent-input" value="${toHex(currentAccent)}" />
            <button class="btn" id="reset-accent-btn">Restablecer</button>
          </div>
          <div id="accent-warning" style="display:none"></div>
        </div>

        ${
          !isGuiManaged
            ? `<div class="empty-state disclosure" style="margin-top:16px; text-align:left">
                 Ya tenés un archivo de tema personalizado en <code>${config.theme_path}</code>.
                 Guardar acá lo va a reemplazar por el generado desde esta pantalla.
               </div>`
            : ''
        }

        <div class="panel-row" style="margin-top:20px">
          <span></span>
          <button class="btn btn-primary" id="save-theme-btn">Guardar</button>
        </div>
      </div>
    </div>
  `;

  const accentInput = container.querySelector<HTMLInputElement>('#accent-input')!;
  const accentWarning = container.querySelector<HTMLElement>('#accent-warning')!;
  const themeRadios = container.querySelectorAll<HTMLInputElement>('input[name="base-theme"]');

  function selectedTheme(): 'dark' | 'light' {
    return Array.from(themeRadios).find((r) => r.checked)?.value === 'light' ? 'light' : 'dark';
  }

  function updateWarning(): void {
    const distance = colorDistance(accentInput.value, DANGER_COLOR[selectedTheme()]);
    if (distance < SIMILARITY_WARN_DISTANCE) {
      accentWarning.style.display = 'block';
      accentWarning.className = 'empty-state disclosure';
      accentWarning.style.textAlign = 'left';
      accentWarning.style.marginTop = '8px';
      accentWarning.textContent =
        'Este acento queda muy parecido al rojo de "peligro" (usado en botones destructivos, como reiniciar el torneo o vaciar el índice global) — te va a costar distinguirlos a simple vista.';
    } else {
      accentWarning.style.display = 'none';
    }
  }

  function preview(): void {
    injectStyle(PREVIEW_STYLE_ID, accentCss(selectedTheme(), accentInput.value));
    updateWarning();
  }

  // El tema base (dark.css/light.css completo) no tiene vista previa en
  // vivo sin persistir — cambiar radios recién aplica al guardar, para no
  // reinyectar los ~25 tokens del tema entero en cada click. El acento sí
  // se previsualiza en vivo porque es una sola variable.
  accentInput.addEventListener('input', preview);
  themeRadios.forEach((r) => r.addEventListener('change', updateWarning));
  updateWarning();

  container.querySelector('#reset-accent-btn')?.addEventListener('click', () => {
    accentInput.value = toHex(DEFAULT_ACCENT[selectedTheme()]);
    preview();
  });

  container.querySelector('#save-theme-btn')?.addEventListener('click', async () => {
    if (!isGuiManaged) {
      const confirmed = await confirmDialog({
        title: 'Reemplazar tema personalizado',
        message: `Ya tenés un archivo de tema en "${config.theme_path}" que no fue generado por esta pantalla. Guardar acá lo va a reemplazar.`,
        confirmLabel: 'Reemplazar y guardar',
        danger: true,
      });
      if (!confirmed) return;
    }
    try {
      const theme = selectedTheme();
      await invoke('write_theme_config', { theme });
      await invoke('write_theme_override', { css: accentCss(theme, accentInput.value) });
      document.documentElement.dataset.theme = theme;
      showToast('Ajustes guardados');
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e), true);
    }
  });
}

/** `<input type=color>` exige `#rrggbb` — normaliza nombres/formatos cortos
 *  si algún día el acento persistido no viene ya en ese formato. */
function toHex(color: string): string {
  if (/^#[0-9a-fA-F]{6}$/.test(color)) return color;
  const probe = document.createElement('div');
  probe.style.color = color;
  document.body.appendChild(probe);
  const rgb = getComputedStyle(probe).color;
  document.body.removeChild(probe);
  const match = rgb.match(/\d+/g);
  if (!match) return DEFAULT_ACCENT.dark;
  const [r, g, b] = match.map(Number);
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}
