// Notificación transitoria — delega a Sonner (`<Toaster />` montado una vez
// en App.tsx). Firma sin cambios respecto a la implementación anterior (DOM
// manual) a propósito: ~44 call sites en 7 vistas llaman `showToast(message,
// isError?)` y no deberían tener que saber qué librería hay atrás.
import { toast } from 'sonner';

export function showToast(message: string, isError = false): void {
  if (isError) {
    toast.error(message);
  } else {
    toast(message);
  }
}
