// Cache en memoria de miniaturas ya decodificadas (data URL), para no volver
// a invocar `get-thumbnail` por cada re-render de una imagen ya vista en esta
// sesión. Se limpia al cambiar de proyecto (ver state.ts).
import { cli, CliError } from './index';

const cache = new Map<string, string | null>();

function key(dbPath: string, imageId: number): string {
  return `${dbPath}::${imageId}`;
}

/** Devuelve un `data:image/jpeg;base64,...` o `null` si la miniatura falló. */
export async function getThumbnailDataUrl(
  dbPath: string,
  imageId: number,
): Promise<string | null> {
  const k = key(dbPath, imageId);
  if (cache.has(k)) return cache.get(k)!;

  try {
    const result = await cli.getThumbnail(dbPath, imageId);
    const url = `data:image/jpeg;base64,${result.thumbnail_b64}`;
    cache.set(k, url);
    return url;
  } catch (e) {
    if (e instanceof CliError) {
      cache.set(k, null);
      return null;
    }
    throw e;
  }
}

const previewCache = new Map<string, string | null>();

/**
 * Igual que `getThumbnailDataUrl` pero pide `get-preview` (re-decodifica el
 * archivo original a `preview_zoom_size`, más grande) — usada solo por el
 * Lightbox al hacer zoom, nunca en grillas/listas.
 */
export async function getPreviewDataUrl(
  dbPath: string,
  imageId: number,
): Promise<string | null> {
  const k = key(dbPath, imageId);
  if (previewCache.has(k)) return previewCache.get(k)!;

  try {
    const result = await cli.getPreview(dbPath, imageId);
    const url = `data:image/jpeg;base64,${result.preview_b64}`;
    previewCache.set(k, url);
    return url;
  } catch (e) {
    if (e instanceof CliError) {
      previewCache.set(k, null);
      return null;
    }
    throw e;
  }
}

export function clearThumbnailCache(): void {
  cache.clear();
  previewCache.clear();
}
