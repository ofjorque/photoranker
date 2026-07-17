// Visor de imagen grande con zoom, reutilizable en cualquier lugar que
// muestre miniaturas (torneo, ráfagas, clusters, clasificación visual) — ver
// docs/fase5-gui.md, agregado por feedback de uso real. La miniatura en sí
// sigue acotada a `preview_size` (ver config.md), así que el zoom agranda en
// pantalla lo que ya existe, no revela más resolución real del archivo.
// API imperativa (`openLightbox(src, alt) `, igual que antes de la
// migración a React) montando un root ad hoc — mismo patrón que
// `confirmDialog()`/`showLoadingOverlay()`, para poder abrirlo desde un
// simple `onClick` en cualquier vista sin tener que montarlo con JSX.
import { createRoot } from 'react-dom/client';
import { useEffect, useRef, useState } from 'react';
import { X, ZoomIn, ZoomOut, RotateCcw } from 'lucide-react';
import { cycleFocus } from './focusTrap';
import { t } from '@/i18n';

const MIN_SCALE = 1;
const MAX_SCALE = 5;
const SCALE_STEP = 0.4;

function LightboxView({ src, alt, onClose }: { src: string; alt: string; onClose: () => void }) {
  const [scale, setScaleState] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const imgRef = useRef<HTMLImageElement>(null);
  const closeBtnRef = useRef<HTMLButtonElement>(null);
  const zoomOutRef = useRef<HTMLButtonElement>(null);
  const zoomInRef = useRef<HTMLButtonElement>(null);
  const resetRef = useRef<HTMLButtonElement>(null);
  const dragStart = useRef({ x: 0, y: 0, panX: 0, panY: 0 });

  const setScale = (next: number) => {
    const clamped = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    setScaleState(clamped);
    if (clamped === MIN_SCALE) setPan({ x: 0, y: 0 });
  };

  useEffect(() => {
    closeBtnRef.current?.focus();
  }, []);

  // Foco atrapado dentro del overlay (mismo criterio que ConfirmDialog).
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        onClose();
        return;
      }
      if (e.key === '+' || e.key === '=') setScale(scale + SCALE_STEP);
      else if (e.key === '-') setScale(scale - SCALE_STEP);
      else if (e.key === '0') setScale(1);
      else if (e.key === 'Tab') {
        e.preventDefault();
        const focusable = [zoomOutRef.current, resetRef.current, zoomInRef.current, closeBtnRef.current].filter(
          (el): el is HTMLButtonElement => el !== null,
        );
        cycleFocus(focusable, e.shiftKey);
      }
    }
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [scale, onClose]);

  useEffect(() => {
    function onMouseMove(e: MouseEvent) {
      if (!dragging) return;
      setPan({
        x: dragStart.current.panX + (e.clientX - dragStart.current.x),
        y: dragStart.current.panY + (e.clientY - dragStart.current.y),
      });
    }
    function onMouseUp() {
      setDragging(false);
    }
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
    return () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };
  }, [dragging]);

  return (
    <div
      className="fixed inset-0 z-[3000] flex items-center justify-center bg-black/86 cursor-zoom-out"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="w-[90vw] h-[90vh] flex items-center justify-center overflow-hidden cursor-default"
        onWheel={(e) => {
          e.preventDefault();
          setScale(scale + (e.deltaY < 0 ? SCALE_STEP : -SCALE_STEP));
        }}
      >
        <img
          ref={imgRef}
          src={src}
          alt={alt}
          className={dragging ? 'cursor-grabbing select-none' : scale > 1 ? 'cursor-grab select-none' : 'select-none'}
          style={{
            maxWidth: '100%',
            maxHeight: '100%',
            transform: `translate(${pan.x}px, ${pan.y}px) scale(${scale})`,
            transition: dragging ? 'none' : 'transform 0.06s linear',
          }}
          onMouseDown={(e) => {
            if (scale <= 1) return;
            e.preventDefault();
            dragStart.current = { x: e.clientX, y: e.clientY, panX: pan.x, panY: pan.y };
            setDragging(true);
          }}
        />
      </div>

      {/* El overlay siempre es oscuro sin importar el tema activo, así que el
          texto siempre necesita ser claro — nunca text-foreground, que en
          tema claro sería casi negro e ilegible acá. */}
      <div className="fixed top-[22px] left-1/2 -translate-x-1/2 text-white/70 text-xs z-[3001]">
        {t('lightbox.hint')}
      </div>

      <button
        ref={closeBtnRef}
        onClick={onClose}
        title={t('lightbox.closeTitle')}
        className="fixed top-5 right-6 z-[3001] w-9 h-9 flex items-center justify-center rounded-md border bg-background text-foreground hover:border-destructive hover:text-destructive"
      >
        <X className="w-4 h-4" />
      </button>

      <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[3001] flex gap-2 glass shadow-lg border rounded-lg p-2">
        <button
          ref={zoomOutRef}
          onClick={() => setScale(scale - SCALE_STEP)}
          title={t('lightbox.zoomOut')}
          className="w-9 h-9 flex items-center justify-center rounded-md text-foreground hover:bg-accent"
        >
          <ZoomOut className="w-4 h-4" />
        </button>
        <button
          ref={resetRef}
          onClick={() => setScale(1)}
          title={t('lightbox.zoomReset')}
          className="w-9 h-9 flex items-center justify-center rounded-md text-foreground hover:bg-accent"
        >
          <RotateCcw className="w-4 h-4" />
        </button>
        <button
          ref={zoomInRef}
          onClick={() => setScale(scale + SCALE_STEP)}
          title={t('lightbox.zoomIn')}
          className="w-9 h-9 flex items-center justify-center rounded-md text-foreground hover:bg-accent"
        >
          <ZoomIn className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}

export function openLightbox(src: string, alt = ''): void {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  const close = () => {
    root.unmount();
    container.remove();
  };

  root.render(<LightboxView src={src} alt={alt} onClose={close} />);
}
