/** Cicla el foco (Tab/Shift+Tab) entre `focusable`, atrapándolo dentro de un
 *  modal en vez de dejarlo escapar hacia la página de atrás — usado por
 *  ConfirmDialog.ts y Lightbox.ts, que comparten exactamente esta mecánica. */
export function cycleFocus(focusable: HTMLElement[], shiftKey: boolean): void {
  const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
  const nextIndex = shiftKey
    ? (currentIndex - 1 + focusable.length) % focusable.length
    : (currentIndex + 1) % focusable.length;
  focusable[nextIndex].focus();
}
