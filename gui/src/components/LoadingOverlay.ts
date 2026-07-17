// Overlay de espera para operaciones lentas (init, cluster --preview/--k).
// Por defecto muestra un spinner + fases rotativas (el CLI imprime una sola
// línea JSON al terminar, ver docs/conventions.md, así que no hay progreso
// real que reportar sin cambiar ese contrato). Cuando la operación es la
// versión "async" con streaming de logs (ver api/asyncCli.ts), `setStatus`
// reemplaza las fases canned por el texto real más reciente (ej. el archivo
// que se está procesando) y agrega un botón Cancelar — ver
// docs/fase5-gui.md, agregado por feedback de uso real.
import './loadingOverlay.css';
import { icons } from './icons';

export interface LoadingHandle {
  close: () => void;
  /** Reemplaza el texto de fase por uno real (ej. "Procesando: IMG_1234.CR3") y detiene la rotación canned. */
  setStatus: (text: string) => void;
  /** Reemplaza el spinner por un estado explícito de "listo" (ícono +
   *  mensaje + botón Cerrar) en vez de cerrar el overlay directamente — para
   *  operaciones donde el usuario quiere una confirmación visible de que
   *  terminó, no solo un toast que desaparece solo (feedback de uso real
   *  sobre `export-xmp`: "debería haber un diálogo que... diga que todo
   *  finalizó"). Resuelve cuando el usuario cierra (botón, Enter o Escape). */
  finish: (message: string) => Promise<void>;
}

export interface LoadingOverlayOptions {
  onCancel?: () => void;
}

export function showLoadingOverlay(
  title: string,
  phases: string[],
  options: LoadingOverlayOptions = {},
): LoadingHandle {
  const overlay = document.createElement('div');
  overlay.className = 'loading-overlay';
  overlay.innerHTML = `
    <div class="loading-card">
      <div class="loading-spinner"></div>
      <div class="loading-title">${title}</div>
      <div class="loading-phase"></div>
      <div class="loading-elapsed"></div>
      ${options.onCancel ? '<button class="btn btn-danger" id="loading-cancel-btn">Cancelar</button>' : ''}
    </div>
  `;
  document.body.appendChild(overlay);

  const phaseEl = overlay.querySelector<HTMLElement>('.loading-phase')!;
  const elapsedEl = overlay.querySelector<HTMLElement>('.loading-elapsed')!;
  const cancelBtn = overlay.querySelector<HTMLButtonElement>('#loading-cancel-btn');

  const startedAt = Date.now();
  let phaseIndex = 0;
  let liveStatus = false;
  phaseEl.textContent = phases[0] ?? '';

  const phaseTimer = window.setInterval(() => {
    if (liveStatus) return;
    phaseIndex = (phaseIndex + 1) % phases.length;
    phaseEl.style.opacity = '0';
    window.setTimeout(() => {
      phaseEl.textContent = phases[phaseIndex] ?? '';
      phaseEl.style.opacity = '1';
    }, 200);
  }, 2400);

  const elapsedTimer = window.setInterval(() => {
    const seconds = Math.round((Date.now() - startedAt) / 1000);
    elapsedEl.textContent = `${seconds}s`;
  }, 500);

  cancelBtn?.addEventListener('click', () => {
    cancelBtn.disabled = true;
    cancelBtn.textContent = 'Cancelando…';
    options.onCancel?.();
  });

  return {
    close() {
      window.clearInterval(phaseTimer);
      window.clearInterval(elapsedTimer);
      overlay.remove();
    },
    setStatus(text: string) {
      liveStatus = true;
      phaseEl.textContent = text;
      phaseEl.style.opacity = '1';
    },
    finish(message: string) {
      window.clearInterval(phaseTimer);
      window.clearInterval(elapsedTimer);
      return new Promise<void>((resolve) => {
        const card = overlay.querySelector<HTMLElement>('.loading-card')!;
        card.classList.add('loading-card--done');
        card.innerHTML = `
          <div class="loading-done-icon">${icons.check}</div>
          <div class="loading-title">Listo</div>
          <div class="loading-phase" style="opacity:1">${message}</div>
          <button class="btn btn-primary" id="loading-close-btn">Cerrar</button>
        `;
        const closeBtn = card.querySelector<HTMLButtonElement>('#loading-close-btn')!;

        function done() {
          document.removeEventListener('keydown', onKeyDown);
          overlay.remove();
          resolve();
        }
        function onKeyDown(ev: KeyboardEvent) {
          if (ev.key === 'Escape' || ev.key === 'Enter') done();
        }
        closeBtn.addEventListener('click', done);
        document.addEventListener('keydown', onKeyDown);
        closeBtn.focus();
      });
    },
  };
}

/** Envuelve una promesa con el overlay de espera, garantizando el cierre incluso si falla. */
export async function withLoadingOverlay<T>(
  title: string,
  phases: string[],
  task: () => Promise<T>,
): Promise<T> {
  const handle = showLoadingOverlay(title, phases);
  try {
    return await task();
  } finally {
    handle.close();
  }
}

/** Para acciones cortas de un solo llamado al CLI (prune, burst-detect,
 *  tournament-undo/reset, export-xmp) donde el overlay de pantalla completa
 *  sería exagerado: solo deshabilita el botón y cambia su texto mientras la
 *  promesa está en vuelo — evita el doble-click sin la ceremonia de fases
 *  simuladas de `withLoadingOverlay` (ver docs/fase5-gui.md, consistencia de
 *  estados de carga). Restaura el texto y estado original incluso si falla. */
export async function withButtonBusy<T>(
  button: HTMLButtonElement,
  busyText: string,
  task: () => Promise<T>,
): Promise<T> {
  const originalText = button.textContent;
  const originalDisabled = button.disabled;
  button.disabled = true;
  button.textContent = busyText;
  try {
    return await task();
  } finally {
    button.disabled = originalDisabled;
    button.textContent = originalText;
  }
}
