// Tablero de ranking por teclado, compartido por el torneo principal y el
// minitorneo de ráfagas (misma mecánica exacta, ver docs/fase3-torneo.md
// "Interacción por Teclado"):
//   - Flechas / Tab mueven el foco entre miniaturas (borde resaltado = foco).
//   - 1..N asigna posición a la imagen en foco; si la posición ya está
//     ocupada, ambas quedan empatadas.
//   - Enter confirma — bloqueado hasta que todas tengan posición asignada.
//   - Backspace / R reinicia las posiciones del grupo actual.
import './rankingBoard.css';
import { getThumbnailDataUrl } from '../api/thumbnailCache';

export interface RankingBoardImage {
  id: number;
  file_path: string;
}

export interface RankingBoardOptions {
  dbPath: string;
  images: RankingBoardImage[];
  onSubmit: (ranking: Array<[number, number]>) => void;
  /** Texto adicional a mostrar sobre el tablero (ej. mu/sigma), opcional. */
  captionFor?: (img: RankingBoardImage) => string;
  /** Notificado cuando cambia la imagen en foco (ej. para el panel de calidad). */
  onFocusChange?: (img: RankingBoardImage) => void;
}

export function mountRankingBoard(
  container: HTMLElement,
  opts: RankingBoardOptions,
): { destroy: () => void } {
  const { dbPath, images, onSubmit, captionFor, onFocusChange } = opts;
  const positions = new Map<number, number>();
  let focusedIndex = 0;

  container.innerHTML = '';
  const wrap = document.createElement('div');
  wrap.className = 'view';

  const board = document.createElement('div');
  board.className = 'ranking-board';

  const cards: HTMLElement[] = [];

  images.forEach((img, idx) => {
    const card = document.createElement('div');
    card.className = 'ranking-card';
    card.tabIndex = -1;
    card.dataset.index = String(idx);

    const thumbWrap = document.createElement('div');
    thumbWrap.className = 'thumb-wrap';
    const placeholder = document.createElement('div');
    placeholder.className = 'thumb-placeholder';
    placeholder.textContent = 'Cargando…';
    thumbWrap.appendChild(placeholder);
    card.appendChild(thumbWrap);

    const meta = document.createElement('div');
    meta.className = 'meta';
    const name = img.file_path.split(/[\\/]/).pop() ?? img.file_path;
    meta.innerHTML = `<span title="${img.file_path}">${name}</span><span>${
      captionFor ? captionFor(img) : ''
    }</span>`;
    card.appendChild(meta);

    card.addEventListener('click', () => {
      focusedIndex = idx;
      renderFocus();
    });

    board.appendChild(card);
    cards.push(card);

    getThumbnailDataUrl(dbPath, img.id).then((url) => {
      thumbWrap.innerHTML = '';
      if (url) {
        const el = document.createElement('img');
        el.src = url;
        el.alt = name;
        thumbWrap.appendChild(el);
      } else {
        const fail = document.createElement('div');
        fail.className = 'thumb-placeholder';
        fail.textContent = 'Sin miniatura';
        thumbWrap.appendChild(fail);
      }
    });
  });

  const footer = document.createElement('div');
  footer.className = 'ranking-board-footer';
  const hint = document.createElement('div');
  hint.className = 'ranking-hint';
  hint.innerHTML =
    '<kbd>&larr;&rarr;</kbd>/<kbd>Tab</kbd> mover foco · <kbd>1</kbd>-<kbd>' +
    images.length +
    '</kbd> asignar posición · <kbd>Enter</kbd> confirmar · <kbd>Backspace</kbd>/<kbd>R</kbd> reiniciar';
  const message = document.createElement('div');
  message.className = 'ranking-hint';
  message.style.color = 'var(--color-danger)';
  const submitBtn = document.createElement('button');
  submitBtn.className = 'btn btn-primary';
  submitBtn.textContent = 'Confirmar (Enter)';
  submitBtn.addEventListener('click', trySubmit);

  footer.appendChild(hint);
  footer.appendChild(message);
  footer.appendChild(submitBtn);

  wrap.appendChild(board);
  wrap.appendChild(footer);
  container.appendChild(wrap);

  function renderFocus() {
    cards.forEach((c, i) => c.classList.toggle('focused', i === focusedIndex));
    cards[focusedIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    onFocusChange?.(images[focusedIndex]);
  }

  function renderBadges() {
    const countByPosition = new Map<number, number>();
    for (const pos of positions.values()) {
      countByPosition.set(pos, (countByPosition.get(pos) ?? 0) + 1);
    }
    cards.forEach((card, idx) => {
      const existing = card.querySelector('.position-badge');
      if (existing) existing.remove();
      const img = images[idx];
      const pos = positions.get(img.id);
      if (pos == null) return;
      const badge = document.createElement('div');
      const tied = (countByPosition.get(pos) ?? 0) > 1;
      badge.className = 'position-badge' + (tied ? ' tie' : '');
      badge.textContent = tied ? `=${pos}` : String(pos);
      badge.title = tied ? `Empate en posición ${pos}` : `Posición ${pos}`;
      card.appendChild(badge);
    });
  }

  function assignPositionValue(imageId: number, position: number) {
    positions.set(imageId, position);
    renderBadges();
    message.textContent = '';
  }

  function resetPositions() {
    positions.clear();
    renderBadges();
    message.textContent = '';
  }

  function trySubmit() {
    const missing = images.filter((img) => !positions.has(img.id));
    if (missing.length > 0) {
      message.textContent = `Faltan ${missing.length} imagen(es) por ordenar`;
      return;
    }
    const ranking: Array<[number, number]> = images.map((img) => [img.id, positions.get(img.id)!]);
    onSubmit(ranking);
  }

  function moveFocus(delta: number) {
    focusedIndex = (focusedIndex + delta + images.length) % images.length;
    renderFocus();
  }

  function isTypingTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    const tag = target.tagName;
    return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable;
  }

  function onKeyDown(e: KeyboardEvent) {
    // Nunca interceptar atajos mientras el usuario escribe en un campo de
    // texto en cualquier parte de la app (ver bug: Backspace no funcionaba
    // en "agregar variable" porque este listener global lo capturaba).
    if (isTypingTarget(e.target)) return;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
      e.preventDefault();
      moveFocus(1);
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
      e.preventDefault();
      moveFocus(-1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      trySubmit();
    } else if (e.key === 'Backspace' || e.key.toLowerCase() === 'r') {
      e.preventDefault();
      resetPositions();
    } else {
      const n = Number(e.key);
      if (Number.isInteger(n) && n >= 1 && n <= images.length) {
        e.preventDefault();
        assignPositionValue(images[focusedIndex].id, n);
      }
    }
  }

  document.addEventListener('keydown', onKeyDown);
  renderFocus();

  return {
    destroy() {
      document.removeEventListener('keydown', onKeyDown);
    },
  };
}
