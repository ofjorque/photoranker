// Modal de confirmación propio — reemplaza el confirm() nativo del navegador
// (rompía la inmersión visual del tema custom) para las acciones
// destructivas de Home.ts. Mismo patrón de overlay que Lightbox.ts, pero con
// foco atrapado dentro del modal (gap que tenía Lightbox, corregido acá
// primero por ser el componente nuevo).
import './confirmDialog.css';
import { cycleFocus } from './focusTrap';
import { t } from '../i18n';

export interface ConfirmDialogOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

export function confirmDialog(opts: ConfirmDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'confirm-overlay';

    const panel = document.createElement('div');
    panel.className = 'confirm-panel';
    panel.setAttribute('role', 'alertdialog');
    panel.setAttribute('aria-modal', 'true');
    panel.innerHTML = `
      <h2 class="confirm-title">${opts.title}</h2>
      <p class="confirm-message">${opts.message}</p>
      <div class="confirm-actions">
        <button class="btn" id="confirm-cancel">${opts.cancelLabel ?? t('common.cancel')}</button>
        <button class="btn ${opts.danger ? 'btn-danger' : 'btn-primary'}" id="confirm-ok">${
          opts.confirmLabel ?? t('common.confirm')
        }</button>
      </div>
    `;
    overlay.appendChild(panel);
    document.body.appendChild(overlay);

    const cancelBtn = panel.querySelector<HTMLButtonElement>('#confirm-cancel')!;
    const okBtn = panel.querySelector<HTMLButtonElement>('#confirm-ok')!;
    const focusable = [cancelBtn, okBtn];
    let settled = false;

    function settle(result: boolean) {
      if (settled) return;
      settled = true;
      document.removeEventListener('keydown', onKeyDown);
      overlay.remove();
      resolve(result);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        settle(false);
        return;
      }
      if (e.key !== 'Tab') return;
      // Foco atrapado dentro del modal — Tab/Shift+Tab ciclan entre los dos
      // botones en vez de escapar hacia la página de atrás (gap que sí tenía
      // Lightbox.ts, ver focusTrap.ts, compartido por ambos).
      e.preventDefault();
      cycleFocus(focusable, e.shiftKey);
    }

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) settle(false);
    });
    cancelBtn.addEventListener('click', () => settle(false));
    okBtn.addEventListener('click', () => settle(true));
    document.addEventListener('keydown', onKeyDown);

    okBtn.focus();
  });
}
