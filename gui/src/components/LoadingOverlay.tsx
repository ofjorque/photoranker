// Overlay de espera para operaciones sin progreso real reportado por el CLI
// (init, cluster --preview/--k): el CLI imprime una sola línea JSON al
// terminar (ver docs/conventions.md), así que no hay % real que mostrar sin
// cambiar ese contrato — spinner indeterminado + texto rotativo de fases
// típicas. API imperativa (`showLoadingOverlay(...) -> handle`, igual que
// antes de la migración a React) montando un root de React ad hoc, mismo
// patrón que `confirmDialog()` en ConfirmDialog.tsx — así los call sites de
// cada vista no necesitan convertirse a JSX para mostrar el overlay.
import { createRoot } from 'react-dom/client';
import { useEffect, useRef, useState } from 'react';
import { CheckCircle2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { t } from '@/i18n';

export interface LoadingOverlayOptions {
  onCancel?: () => void;
}

export interface LoadingHandle {
  close: () => void;
  /** Reemplaza el texto de fase por uno real (ej. "Procesando: IMG_1234.CR3") y detiene la rotación canned. */
  setStatus: (text: string) => void;
  /** Reemplaza el spinner por un estado explícito de "listo" (ícono + mensaje
   *  + botón Cerrar) en vez de cerrar el overlay directamente — para
   *  operaciones donde el usuario quiere una confirmación visible de que
   *  terminó, no solo un toast que desaparece solo (feedback de uso real
   *  sobre `export-xmp`). Resuelve cuando el usuario cierra (botón, Enter o Escape). */
  finish: (message: string) => Promise<void>;
}

interface Controller {
  setStatus: (text: string) => void;
  finish: (message: string) => void;
}

function LoadingOverlayView({
  title,
  phases,
  onCancel,
  controllerRef,
  onClose,
}: {
  title: string;
  phases: string[];
  onCancel?: () => void;
  controllerRef: { current: Controller | null };
  onClose: () => void;
}) {
  const [phaseIndex, setPhaseIndex] = useState(0);
  const [liveStatus, setLiveStatus] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [cancelling, setCancelling] = useState(false);
  const [doneMessage, setDoneMessage] = useState<string | null>(null);
  const startedAt = useRef(Date.now());

  useEffect(() => {
    controllerRef.current = {
      setStatus: setLiveStatus,
      finish: setDoneMessage,
    };
  });

  useEffect(() => {
    if (doneMessage !== null) return;
    const elapsedTimer = window.setInterval(() => {
      setElapsed(Math.round((Date.now() - startedAt.current) / 1000));
    }, 500);
    return () => window.clearInterval(elapsedTimer);
  }, [doneMessage]);

  useEffect(() => {
    if (doneMessage !== null || liveStatus !== null) return;
    const phaseTimer = window.setInterval(() => {
      setPhaseIndex((i) => (i + 1) % phases.length);
    }, 2400);
    return () => window.clearInterval(phaseTimer);
  }, [doneMessage, liveStatus, phases.length]);

  useEffect(() => {
    if (doneMessage === null) return;
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' || e.key === 'Enter') onClose();
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [doneMessage, onClose]);

  if (doneMessage !== null) {
    return (
      <div className="fixed inset-0 z-[2000] flex items-center justify-center bg-background/80 backdrop-blur-sm">
        <div className="glass shadow-lg border rounded-lg p-8 flex flex-col items-center gap-3 min-w-[320px] max-w-[360px] text-center">
          <CheckCircle2 className="w-10 h-10 text-success" />
          <div className="font-semibold text-sm">{t('loadingOverlay.done')}</div>
          <div className="text-sm">{doneMessage}</div>
          <Button className="mt-2" autoFocus onClick={onClose}>
            {t('common.close')}
          </Button>
        </div>
      </div>
    );
  }

  const phaseText = liveStatus ?? phases[phaseIndex] ?? '';

  return (
    <div className="fixed inset-0 z-[2000] flex items-center justify-center bg-background/80 backdrop-blur-sm">
      <div className="glass shadow-lg border rounded-lg p-8 flex flex-col items-center gap-3 min-w-[280px] max-w-[360px] text-center">
        <div className="w-11 h-11 rounded-full border-[3px] border-border border-t-primary animate-spin" />
        <div className="font-semibold text-sm">{title}</div>
        <div className="text-muted-foreground text-xs min-h-[16px]">{phaseText}</div>
        <div className="text-muted-foreground text-[11px] tabular-nums">{elapsed}s</div>
        {onCancel && (
          <Button
            variant="destructive"
            disabled={cancelling}
            onClick={() => {
              setCancelling(true);
              onCancel();
            }}
          >
            {cancelling ? t('loadingOverlay.cancelling') : t('common.cancel')}
          </Button>
        )}
      </div>
    </div>
  );
}

export function showLoadingOverlay(
  title: string,
  phases: string[],
  options: LoadingOverlayOptions = {},
): LoadingHandle {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  const controllerRef: { current: Controller | null } = { current: null };

  let unmounted = false;
  let finishResolve: (() => void) | null = null;

  const onClose = () => {
    if (unmounted) return;
    unmounted = true;
    root.unmount();
    container.remove();
    finishResolve?.();
  };

  root.render(
    <LoadingOverlayView
      title={title}
      phases={phases}
      onCancel={options.onCancel}
      controllerRef={controllerRef}
      onClose={onClose}
    />,
  );

  return {
    close: onClose,
    setStatus(text: string) {
      controllerRef.current?.setStatus(text);
    },
    finish(message: string) {
      controllerRef.current?.finish(message);
      return new Promise<void>((resolve) => {
        finishResolve = resolve;
      });
    },
  };
}

/** Envuelve una promesa con el overlay de espera, garantizando el cierre incluso si falla. */
export async function withLoadingOverlay<T>(
  title: string,
  phases: string[],
  task: () => Promise<T>,
): Promise<T> {
  const handle = showLoadingOverlay(title, phases);
  try {
    return await task();
  } finally {
    handle.close();
  }
}
