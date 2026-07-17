// Selector de idioma es/en — ver docs/fase5-gui.md, "Internacionalización
// (i18n)". Diccionarios planos, sin librería externa (mismo criterio del
// resto de la Fase 5: vanilla TS). NO traduce el texto que la GUI parsea de
// stdout/stderr del propio CLI (siempre en español, ver api/asyncCli.ts) —
// solo el copy propio de gui/src/**.
import { invoke } from '@tauri-apps/api/core';
import { es } from './es';
import { en } from './en';

export type Language = 'es' | 'en';

const dictionaries: Record<Language, Record<string, string>> = { es, en };

let language: Language = 'es';
const listeners = new Set<() => void>();

export function getLanguage(): Language {
  return language;
}

export function setLanguage(lang: Language): void {
  language = lang;
  document.documentElement.lang = lang;
  listeners.forEach((fn) => fn());
}

export function onLanguageChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Busca `key` en el idioma activo; si falta, cae a español; si tampoco
 *  existe ahí, devuelve la clave entre corchetes y avisa por consola — una
 *  clave faltante nunca debe romper el render (mismo criterio de fallback
 *  silencioso que `theme/index.ts`). Interpola `{var}` con `vars`. */
export function t(key: string, vars?: Record<string, string | number>): string {
  let template = dictionaries[language][key] ?? dictionaries.es[key];
  if (template === undefined) {
    console.warn(`[i18n] clave faltante: ${key}`);
    return `[[${key}]]`;
  }
  if (vars) {
    for (const [name, value] of Object.entries(vars)) {
      template = template.split(`{${name}}`).join(String(value));
    }
  }
  return template;
}

interface LanguageConfig {
  language: string;
}

async function getLanguageConfig(): Promise<LanguageConfig> {
  try {
    return await invoke<LanguageConfig>('read_language_config');
  } catch {
    return { language: 'es' };
  }
}

export async function initLanguage(): Promise<void> {
  const config = await getLanguageConfig();
  setLanguage(config.language === 'en' ? 'en' : 'es');
}

if (import.meta.env.DEV) {
  const missingInEn = Object.keys(es).filter((k) => !(k in en));
  const missingInEs = Object.keys(en).filter((k) => !(k in es));
  if (missingInEn.length > 0 || missingInEs.length > 0) {
    console.warn('[i18n] diccionarios es/en desincronizados', { missingInEn, missingInEs });
  }
}
