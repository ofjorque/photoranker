// Visor de imagen grande con zoom, reutilizable en cualquier lugar que
// muestre miniaturas (torneo, ráfagas, clusters, clasificación visual) — ver
// docs/fase5-gui.md, agregado por feedback de uso real. La miniatura en sí
// sigue acotada a `preview_size` (ver config.md), así que el zoom agranda en
// pantalla lo que ya existe, no revela más resolución real del archivo.
import './lightbox.css';
import { icons } from './icons';

const MIN_SCALE = 1;
const MAX_SCALE = 5;
const SCALE_STEP = 0.4;

export function openLightbox(src: string, alt = ''): void {
  const overlay = document.createElement('div');
  overlay.className = 'lightbox-overlay';

  const hint = document.createElement('div');
  hint.className = 'lightbox-hint';
  hint.textContent = 'Rueda del mouse o +/- para zoom · arrastrar para mover · Esc para cerrar';

  const closeBtn = document.createElement('button');
  closeBtn.className = 'lightbox-close';
  closeBtn.innerHTML = icons.close;
  closeBtn.title = 'Cerrar (Esc)';

  const stage = document.createElement('div');
  stage.className = 'lightbox-stage';

  const img = document.createElement('img');
  img.className = 'lightbox-img';
  img.src = src;
  img.alt = alt;
  stage.appendChild(img);

  const controls = document.createElement('div');
  controls.className = 'lightbox-controls';
  const zoomOutBtn = document.createElement('button');
  zoomOutBtn.innerHTML = icons.zoomOut;
  zoomOutBtn.title = 'Alejar';
  const resetBtn = document.createElement('button');
  resetBtn.innerHTML = icons.zoomReset;
  resetBtn.title = 'Restablecer zoom (1:1)';
  const zoomInBtn = document.createElement('button');
  zoomInBtn.innerHTML = icons.zoomIn;
  zoomInBtn.title = 'Acercar';
  controls.appendChild(zoomOutBtn);
  controls.appendChild(resetBtn);
  controls.appendChild(zoomInBtn);

  overlay.appendChild(stage);
  overlay.appendChild(hint);
  overlay.appendChild(closeBtn);
  overlay.appendChild(controls);
  document.body.appendChild(overlay);

  let scale = 1;
  let panX = 0;
  let panY = 0;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let panStartX = 0;
  let panStartY = 0;

  function applyTransform() {
    img.style.transform = `translate(${panX}px, ${panY}px) scale(${scale})`;
    img.classList.toggle('zoomed', scale > 1);
  }

  function setScale(next: number) {
    scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    if (scale === MIN_SCALE) {
      panX = 0;
      panY = 0;
    }
    applyTransform();
  }

  function close() {
    document.removeEventListener('keydown', onKeyDown);
    overlay.remove();
  }

  // Foco atrapado dentro del overlay (gap identificado en el reporte de
  // exploración de la GUI: Tab podía escapar hacia la página de atrás) —
  // mismo criterio que ConfirmDialog.ts.
  const focusable = [zoomOutBtn, resetBtn, zoomInBtn, closeBtn];

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      close();
      return;
    }
    if (e.key === '+' || e.key === '=') setScale(scale + SCALE_STEP);
    else if (e.key === '-') setScale(scale - SCALE_STEP);
    else if (e.key === '0') setScale(1);
    else if (e.key === 'Tab') {
      const currentIndex = focusable.indexOf(document.activeElement as HTMLButtonElement);
      e.preventDefault();
      const nextIndex = e.shiftKey
        ? (currentIndex - 1 + focusable.length) % focusable.length
        : (currentIndex + 1) % focusable.length;
      focusable[nextIndex].focus();
    }
  }

  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });
  closeBtn.addEventListener('click', close);
  zoomInBtn.addEventListener('click', () => setScale(scale + SCALE_STEP));
  zoomOutBtn.addEventListener('click', () => setScale(scale - SCALE_STEP));
  resetBtn.addEventListener('click', () => setScale(1));

  stage.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      setScale(scale + (e.deltaY < 0 ? SCALE_STEP : -SCALE_STEP));
    },
    { passive: false },
  );

  img.addEventListener('mousedown', (e) => {
    if (scale <= 1) return;
    dragging = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    panStartX = panX;
    panStartY = panY;
    img.classList.add('panning');
    e.preventDefault();
  });
  window.addEventListener('mousemove', (e) => {
    if (!dragging) return;
    panX = panStartX + (e.clientX - dragStartX);
    panY = panStartY + (e.clientY - dragStartY);
    applyTransform();
  });
  window.addEventListener('mouseup', () => {
    dragging = false;
    img.classList.remove('panning');
  });

  document.addEventListener('keydown', onKeyDown);
  closeBtn.focus();
}

/** Marca `el` como clickeable para abrir `src` en el lightbox (agrega el cursor de zoom-in). */
export function makeZoomable(el: HTMLElement, getSrc: () => string | null, alt = ''): void {
  el.classList.add('zoomable');
  el.addEventListener('click', (e) => {
    e.stopPropagation();
    const src = getSrc();
    if (src) openLightbox(src, alt);
  });
}
