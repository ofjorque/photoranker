// Overlay de espera indeterminado para operaciones lentas sin progreso real
// (init, cluster --preview/--k, que corre R vía std::process::Command) — ver
// docs/fase5-gui.md, checklist de la GUI, y loadingOverlay.css para por qué
// no es una barra de % real.
import './loadingOverlay.css';

export interface LoadingHandle {
  close: () => void;
}

export function showLoadingOverlay(title: string, phases: string[]): LoadingHandle {
  const overlay = document.createElement('div');
  overlay.className = 'loading-overlay';
  overlay.innerHTML = `
    <div class="loading-card">
      <div class="loading-spinner"></div>
      <div class="loading-title">${title}</div>
      <div class="loading-phase"></div>
      <div class="loading-elapsed"></div>
    </div>
  `;
  document.body.appendChild(overlay);

  const phaseEl = overlay.querySelector<HTMLElement>('.loading-phase')!;
  const elapsedEl = overlay.querySelector<HTMLElement>('.loading-elapsed')!;

  const startedAt = Date.now();
  let phaseIndex = 0;
  phaseEl.textContent = phases[0] ?? '';

  const phaseTimer = window.setInterval(() => {
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

  return {
    close() {
      window.clearInterval(phaseTimer);
      window.clearInterval(elapsedTimer);
      overlay.remove();
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
