// Puente tipado hacia el comando Tauri `run_photoranker`, que a su vez
// invoca `photoranker(.exe)` como subproceso (ver docs/conventions.md,
// "API interna"). Este módulo no agrega lógica de negocio — solo tipa el
// sobre JSON estándar y lo desempaqueta.
import { invoke } from '@tauri-apps/api/core';

export class CliError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = 'CliError';
  }
}

interface OkEnvelope<T> {
  status: 'ok';
  data: T;
}
interface ErrEnvelope {
  status: 'error';
  code: string;
  message: string;
}
type Envelope<T> = OkEnvelope<T> | ErrEnvelope;

/** Ejecuta `photoranker <args>` y devuelve `data` si `status="ok"`, o lanza `CliError`. */
export async function callCli<T>(args: string[]): Promise<T> {
  const envelope = await invoke<Envelope<T>>('run_photoranker', { args });
  if (envelope.status === 'error') {
    throw new CliError(envelope.code, envelope.message);
  }
  return envelope.data;
}

/** Convierte pares [id, posición] al formato `id:posición` que usa el CLI. */
export function formatRankingArgs(ranking: Array<[number, number]>): string[] {
  return ranking.map(([id, pos]) => `${id}:${pos}`);
}
