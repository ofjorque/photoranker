// Notificación transitoria simple, para errores/confirmaciones de comandos.
let timer: ReturnType<typeof setTimeout> | null = null;

export function showToast(message: string, isError = false): void {
  let el = document.getElementById('app-toast');
  if (!el) {
    el = document.createElement('div');
    el.id = 'app-toast';
    document.body.appendChild(el);
  }
  el.className = 'toast' + (isError ? ' toast-error' : '');
  el.textContent = message;
  el.style.display = 'block';

  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (el) el.style.display = 'none';
  }, 4000);
}
