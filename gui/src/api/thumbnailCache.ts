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

export function clearThumbnailCache(): void {
  cache.clear();
}
