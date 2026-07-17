// Drawer de "ver detalles" de una foto — usado en Export (fila del ranking),
// Bursts (miniaturas del panel de exclusión) y Cluster (imágenes
// representativas), lugares que hoy no tienen ningún panel de metadata (a
// diferencia de Torneo, que ya tiene un panel de calidad persistente en la
// columna lateral — ese NO se toca, ver docs/fase5-gui.md). Reusa
// `QualityPanel` tal cual para las métricas, cero lógica de fetch duplicada.
import { useEffect, useState } from 'react';
import { getThumbnailDataUrl } from '@/api/thumbnailCache';
import { QualityPanel } from '@/components/QualityPanel';
import { Button } from '@/components/ui/button';
import { Drawer, DrawerContent, DrawerHeader, DrawerTitle, DrawerFooter, DrawerClose } from '@/components/ui/drawer';
import { t } from '@/i18n';

export interface PhotoDetailsDrawerProps {
  dbPath: string;
  imageId: number | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function PhotoDetailsDrawer({ dbPath, imageId, open, onOpenChange }: PhotoDetailsDrawerProps) {
  const [thumb, setThumb] = useState<string | null>(null);

  useEffect(() => {
    setThumb(null);
    if (!open || imageId === null) return;
    getThumbnailDataUrl(dbPath, imageId).then((url) => setThumb(url));
  }, [open, dbPath, imageId]);

  return (
    <Drawer open={open} onOpenChange={onOpenChange}>
      <DrawerContent>
        <div className="mx-auto w-full max-w-2xl">
          <DrawerHeader>
            <DrawerTitle>{t('photoDetails.title')}</DrawerTitle>
          </DrawerHeader>
          <div className="px-4 pb-4 grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="aspect-[4/3] bg-muted rounded-md overflow-hidden flex items-center justify-center">
              {thumb ? (
                <img src={thumb} alt="" className="w-full h-full object-contain" />
              ) : (
                <span className="text-muted-foreground text-sm">{t('common.loading')}</span>
              )}
            </div>
            <div className="overflow-auto">
              {imageId !== null && <QualityPanel dbPath={dbPath} imageId={imageId} />}
            </div>
          </div>
          <DrawerFooter>
            <DrawerClose asChild>
              <Button variant="outline">{t('photoDetails.close')}</Button>
            </DrawerClose>
          </DrawerFooter>
        </div>
      </DrawerContent>
    </Drawer>
  );
}
