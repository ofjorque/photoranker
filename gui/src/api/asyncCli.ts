// Puente hacia `run_photoranker_async`/`cancel_photoranker` (ver
// gui/src-tauri/src/lib.rs) — versión con streaming de logs y cancelación de
// `callCli`, para comandos lentos (init, cluster --preview/--k) donde la GUI
// quiere mostrar progreso en vivo y poder cortar la espera. Ver
// docs/fase5-gui.md, agregado por feedback de uso real.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { CliError } from './cli';

interface ProcessLogEvent {
  op_id: string;
  stream: 'stdout' | 'stderr';
  line: string;
}

interface ProcessDoneEvent {
  op_id: string;
  envelope: { status: 'ok'; data: unknown } | { status: 'error'; code: string; message: string } | null;
  error: string | null;
  cancelled: boolean;
}

export interface AsyncCliRun<T> {
  promise: Promise<T>;
  cancel: () => Promise<void>;
}

/**
 * Extrae un texto legible de una línea de log de `tracing` (ver formato en
 * core-cli/src/commands/init.rs) — ej. `procesando imagen file=C:\...\a.jpg`
 * → "Procesando: a.jpg". Deliberadamente tolerante: si no reconoce el
 * patrón, devuelve `null` y el llamador simplemente no actualiza el estado
 * (no es un contrato estructurado, es texto de log para mostrar, no para
 * decisiones — ver conventions.md, "la GUI no debe parsear stderr" para
 * lógica; esto es solo despliegue cosmético).
 */
export function extractLogStatus(rawLine: string): string | null {
  // eslint-disable-next-line no-control-regex
  const line = rawLine.replace(/\x1b\[[0-9;]*m/g, '').trim();
  if (line.length === 0) return null;

  const fieldMatch = line.match(/(?:file|path)=(.+)$/);
  const value = fieldMatch?.[1]?.trim().replace(/^"(.*)"$/, '$1');
  const name = value ? (value.split(/[\\/]/).pop() ?? value) : null;

  if (line.includes('procesando imagen') && name) return `Procesando: ${name}`;
  if (line.includes('escaneando carpeta') && value) return `Escaneando: ${value}`;
  const nuevosMatch = line.match(/nuevos=(\d+)/);
  if (line.includes('archivos nuevos encontrados') && nuevosMatch) {
    return `${nuevosMatch[1]} archivos nuevos encontrados…`;
  }
  return null;
}

/**
 * Corre `photoranker <args>` en modo streaming + cancelable. `onLog` recibe
 * cada línea cruda de stdout/stderr (el llamador decide qué mostrar, ej. vía
 * `extractLogStatus`). La promesa resuelve con `data` si `status="ok"`, o
 * rechaza con `CliError` si `status="error"` o si se canceló.
 */
export function runPhotorankerAsync<T>(
  args: string[],
  onLog?: (stream: 'stdout' | 'stderr', line: string) => void,
): AsyncCliRun<T> {
  const opId = crypto.randomUUID();
  let unlistenLog: UnlistenFn | null = null;
  let unlistenDone: UnlistenFn | null = null;

  function cleanup() {
    unlistenLog?.();
    unlistenDone?.();
  }

  const promise = new Promise<T>((resolve, reject) => {
    (async () => {
      unlistenLog = await listen<ProcessLogEvent>('photoranker-log', (event) => {
        if (event.payload.op_id !== opId) return;
        onLog?.(event.payload.stream, event.payload.line);
      });
      unlistenDone = await listen<ProcessDoneEvent>('photoranker-done', (event) => {
        if (event.payload.op_id !== opId) return;
        cleanup();
        const { envelope, error, cancelled } = event.payload;
        if (cancelled) {
          reject(new CliError('CANCELLED', 'Operación cancelada por el usuario'));
          return;
        }
        if (!envelope) {
          reject(new Error(error ?? 'El CLI no produjo salida JSON'));
          return;
        }
        if (envelope.status === 'error') {
          reject(new CliError(envelope.code, envelope.message));
          return;
        }
        resolve(envelope.data as T);
      });
      await invoke('run_photoranker_async', { opId, args });
    })().catch((e) => {
      cleanup();
      reject(e);
    });
  });

  return {
    promise,
    cancel: async () => {
      await invoke('cancel_photoranker', { opId });
    },
  };
}
