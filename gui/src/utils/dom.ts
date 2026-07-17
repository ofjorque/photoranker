/** true si el usuario está escribiendo en un campo de texto — usado para
 *  que los listeners globales de teclado (atajos de torneo/ráfagas,
 *  clasificación de variables) nunca intercepten teclas dentro de un
 *  `<input>`/`<textarea>` (bug real: Backspace no funcionaba en "agregar
 *  variable" porque un listener global de otra vista lo capturaba primero). */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable;
}
