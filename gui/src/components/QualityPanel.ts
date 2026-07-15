// Panel de referencia con métricas objetivas de calidad por imagen (ver
// docs/fase1-ingesta.md sección 2, checklist de fase5-gui.md). Íconos de
// advertencia para baja nitidez / clipping.
import { cli } from '../api';
import type { QualityMetrics } from '../api/types';

const SHARPNESS_WARN_THRESHOLD = 50; // varianza del Laplaciano baja = foto poco nítida
const CLIPPING_WARN_PCT = 5; // % de píxeles clippeados considerado alto

function metricRow(
  label: string,
  value: string,
  warn?: { active: boolean; reason: string },
): string {
  const warnIcon = warn?.active
    ? `<span class="badge badge-danger" title="${warn.reason}">&#9888;</span>`
    : '';
  return `<tr><td>${label}</td><td class="mono">${value}</td><td>${warnIcon}</td></tr>`;
}

export async function renderQualityPanel(
  container: HTMLElement,
  dbPath: string,
  imageId: number,
): Promise<void> {
  container.innerHTML = '<p>Cargando métricas…</p>';
  let metrics: QualityMetrics | null;
  try {
    const result = await cli.getQualityMetrics(dbPath, imageId);
    metrics = result.metrics;
  } catch {
    container.innerHTML = '<p>No se pudieron leer las métricas de calidad.</p>';
    return;
  }

  if (!metrics) {
    container.innerHTML =
      '<p>Sin métricas de calidad (la miniatura de esta imagen falló, ver <code>list-failed-thumbnails</code>).</p>';
    return;
  }

  const rows = [
    metricRow('Nitidez (sharpness)', metrics.sharpness.toFixed(1), {
      active: metrics.sharpness < SHARPNESS_WARN_THRESHOLD,
      reason: 'Varianza del Laplaciano baja: posible foto desenfocada',
    }),
    metricRow('Brillo', metrics.brightness.toFixed(1)),
    metricRow('Contraste', metrics.contrast.toFixed(1)),
    metricRow('Sobreexposición', `${metrics.overexposed_pct.toFixed(1)}%`, {
      active: metrics.overexposed_pct > CLIPPING_WARN_PCT,
      reason: 'Más del 5% de píxeles con clipping por sobreexposición',
    }),
    metricRow('Subexposición', `${metrics.underexposed_pct.toFixed(1)}%`, {
      active: metrics.underexposed_pct > CLIPPING_WARN_PCT,
      reason: 'Más del 5% de píxeles con clipping por subexposición',
    }),
    metricRow('Saturación', metrics.saturation.toFixed(2)),
    metricRow('Colorido (colorfulness)', metrics.colorfulness.toFixed(2)),
    metricRow('Entropía', metrics.entropy.toFixed(2)),
    metricRow(
      'Color promedio',
      `rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b})`,
    ),
    metricRow('Orientación', metrics.orientation),
  ].join('');

  container.innerHTML = `
    <div class="panel-row">
      <span class="swatch" style="display:inline-block;width:16px;height:16px;border-radius:4px;
        background: rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b});
        border:1px solid var(--color-border);"></span>
    </div>
    <table><tbody>${rows}</tbody></table>
  `;
}
