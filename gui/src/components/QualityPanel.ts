// Panel de referencia con métricas objetivas de calidad por imagen (ver
// docs/fase1-ingesta.md sección 2, checklist de fase5-gui.md). Antes era una
// tabla de números puros; ahora cada métrica normalizable tiene una barra
// visual (estilo histograma/medidor, ver reporte de exploración de la GUI:
// "no hay ningún gráfico salvo el scree plot de clustering") además del
// valor numérico exacto, que se sigue mostrando para no perder precisión.
import { cli } from '../api';
import type { QualityMetrics } from '../api/types';
import { icons } from './icons';
import { t } from '../i18n';

const SHARPNESS_WARN_THRESHOLD = 50; // varianza del Laplaciano baja = foto poco nítida
const CLIPPING_WARN_PCT = 5; // % de píxeles clippeados considerado alto

// Máximos "prácticos" para la barra — estas métricas no tienen un tope
// matemático estricto salvo entropía (log2(256)=8) y saturación/porcentajes
// (0-1 / 0-100, ver docs/fase1-ingesta.md sección 2); el resto se clampea a
// un valor de referencia razonable para que la barra sea legible como gauge,
// no como medición científica exacta — una imagen extremadamente nítida o
// colorida simplemente satura la barra al 100%, igual que un VU-meter.
const METER_MAX = {
  sharpness: 300,
  brightness: 255,
  contrast: 128,
  saturation: 1,
  colorfulness: 100,
  entropy: 8,
} as const;

function meterBar(value: number, max: number, warnActive: boolean): string {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  const barClass = warnActive ? 'meter-bar-fill meter-bar-fill--warn' : 'meter-bar-fill';
  return `<div class="meter-bar" role="img" aria-label="${t('qualityPanel.meterAriaLabel', { value: value.toFixed(1), max })}">
    <div class="${barClass}" style="width:${pct}%"></div>
  </div>`;
}

function metricRow(
  label: string,
  valueText: string,
  meter: string | null,
  warn?: { active: boolean; reason: string },
): string {
  const warnIcon = warn?.active
    ? `<span class="badge badge-danger metric-warn" title="${warn.reason}">${icons.warning}</span>`
    : '';
  return `<tr>
    <td>${label}</td>
    <td class="mono">${valueText}</td>
    <td>${meter ?? ''}</td>
    <td>${warnIcon}</td>
  </tr>`;
}

export async function renderQualityPanel(
  container: HTMLElement,
  dbPath: string,
  imageId: number,
): Promise<void> {
  container.innerHTML = `<p>${t('qualityPanel.loading')}</p>`;
  let metrics: QualityMetrics | null;
  try {
    const result = await cli.getQualityMetrics(dbPath, imageId);
    metrics = result.metrics;
  } catch {
    container.innerHTML = `<div class="empty-state">${t('qualityPanel.loadError')}</div>`;
    return;
  }

  if (!metrics) {
    container.innerHTML = `<div class="empty-state">${t('qualityPanel.noMetrics')}</div>`;
    return;
  }

  const sharpnessWarn = metrics.sharpness < SHARPNESS_WARN_THRESHOLD;
  const overWarn = metrics.overexposed_pct > CLIPPING_WARN_PCT;
  const underWarn = metrics.underexposed_pct > CLIPPING_WARN_PCT;

  const rows = [
    metricRow(
      t('qualityPanel.metric.sharpness'),
      metrics.sharpness.toFixed(1),
      meterBar(metrics.sharpness, METER_MAX.sharpness, sharpnessWarn),
      { active: sharpnessWarn, reason: t('qualityPanel.warn.sharpness') },
    ),
    metricRow(
      t('qualityPanel.metric.brightness'),
      metrics.brightness.toFixed(1),
      meterBar(metrics.brightness, METER_MAX.brightness, false),
    ),
    metricRow(
      t('qualityPanel.metric.contrast'),
      metrics.contrast.toFixed(1),
      meterBar(metrics.contrast, METER_MAX.contrast, false),
    ),
    metricRow(
      t('qualityPanel.metric.overexposed'),
      `${metrics.overexposed_pct.toFixed(1)}%`,
      meterBar(metrics.overexposed_pct, 100, overWarn),
      { active: overWarn, reason: t('qualityPanel.warn.overexposed') },
    ),
    metricRow(
      t('qualityPanel.metric.underexposed'),
      `${metrics.underexposed_pct.toFixed(1)}%`,
      meterBar(metrics.underexposed_pct, 100, underWarn),
      { active: underWarn, reason: t('qualityPanel.warn.underexposed') },
    ),
    metricRow(
      t('qualityPanel.metric.saturation'),
      metrics.saturation.toFixed(2),
      meterBar(metrics.saturation, METER_MAX.saturation, false),
    ),
    metricRow(
      t('qualityPanel.metric.colorfulness'),
      metrics.colorfulness.toFixed(2),
      meterBar(metrics.colorfulness, METER_MAX.colorfulness, false),
    ),
    metricRow(
      t('qualityPanel.metric.entropy'),
      metrics.entropy.toFixed(2),
      meterBar(metrics.entropy, METER_MAX.entropy, false),
    ),
    metricRow(
      t('qualityPanel.metric.averageColor'),
      `rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b})`,
      `<span class="swatch" style="background: rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b})"></span>`,
    ),
    metricRow(t('qualityPanel.metric.orientation'), metrics.orientation, null),
  ].join('');

  container.innerHTML = `<table class="quality-table"><tbody>${rows}</tbody></table>`;
}
