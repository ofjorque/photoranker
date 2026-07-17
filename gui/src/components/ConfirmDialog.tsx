import { createRoot } from 'react-dom/client';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { useState } from 'react';
import { t } from '@/i18n';

export interface ConfirmDialogOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

function ConfirmDialogComponent({
  opts,
  onResolve,
  onUnmount
}: {
  opts: ConfirmDialogOptions;
  onResolve: (result: boolean) => void;
  onUnmount: () => void;
}) {
  const [open, setOpen] = useState(true);

  const handleOpenChange = (newOpen: boolean) => {
    setOpen(newOpen);
    if (!newOpen) {
      onResolve(false);
      setTimeout(onUnmount, 300); // wait for animation
    }
  };

  const handleConfirm = () => {
    setOpen(false);
    onResolve(true);
    setTimeout(onUnmount, 300);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{opts.title}</DialogTitle>
          <DialogDescription>{opts.message}</DialogDescription>
        </DialogHeader>
        <DialogFooter className="mt-4">
          <Button variant="outline" onClick={() => handleOpenChange(false)}>
            {opts.cancelLabel ?? t('common.cancel')}
          </Button>
          <Button variant={opts.danger ? "destructive" : "default"} onClick={handleConfirm}>
            {opts.confirmLabel ?? t('common.confirm')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function confirmDialog(opts: ConfirmDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);

    const onUnmount = () => {
      root.unmount();
      container.remove();
    };

    root.render(
      <ConfirmDialogComponent 
        opts={opts} 
        onResolve={resolve} 
        onUnmount={onUnmount} 
      />
    );
  });
}
