// Estado global mínimo de la sesión de la GUI: qué carpeta/BD está abierta.
// No hay lógica de negocio acá — solo la ruta que se pasa como --db a cada
// llamada del CLI (ver docs/conventions.md, "API interna").
import { clearThumbnailCache } from './api/thumbnailCache';

export interface ProjectState {
  folderPath: string;
  dbPath: string;
}

let project: ProjectState | null = null;
const listeners = new Set<() => void>();

export function getProject(): ProjectState | null {
  return project;
}

export function setProject(next: ProjectState | null): void {
  project = next;
  clearThumbnailCache();
  listeners.forEach((fn) => fn());
}

export function onProjectChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** `folderPath` a `<folderPath>/.photoranker.sqlite`, separador correcto por SO. */
export function dbPathFor(folderPath: string): string {
  const sep = folderPath.includes('\\') ? '\\' : '/';
  const trimmed = folderPath.replace(/[\\/]+$/, '');
  return `${trimmed}${sep}.photoranker.sqlite`;
}
