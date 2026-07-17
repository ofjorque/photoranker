import { useState, useEffect } from 'react';
import { cli } from '@/api';
import type { QualityMetrics } from '@/api/types';
import { t } from '@/i18n';
import { AlertCircle } from 'lucide-react';
import { Html } from '@/components/Html';
import { Progress } from '@/components/ui/progress';
import { cn } from '@/lib/utils';

const SHARPNESS_WARN_THRESHOLD = 50;
const CLIPPING_WARN_PCT = 5;

const METER_MAX = {
  sharpness: 300,
  brightness: 255,
  contrast: 128,
  saturation: 1,
  colorfulness: 100,
  entropy: 8,
};

function MeterBar({ value, max, warn }: { value: number; max: number; warn: boolean }) {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <Progress
      value={pct}
      title={value.toFixed(1)}
      className={cn('w-24 h-2', warn && '[&>div]:bg-destructive')}
    />
  );
}

export function QualityPanel({ dbPath, imageId }: { dbPath: string; imageId: number | null }) {
  const [metrics, setMetrics] = useState<QualityMetrics | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (imageId === null) {
      setMetrics(null);
      return;
    }
    setLoading(true);
    setError(false);
    cli.getQualityMetrics(dbPath, imageId)
      .then(res => setMetrics(res.metrics))
      .catch(() => setError(true))
      .finally(() => setLoading(false));
  }, [dbPath, imageId]);

  if (imageId === null) return <p className="text-sm text-muted-foreground">{t('tournament.quality.focusHint')}</p>;
  if (loading) return <p className="text-sm text-muted-foreground">{t('qualityPanel.loading')}</p>;
  if (error) return <p className="text-sm text-destructive">{t('qualityPanel.loadError')}</p>;
  if (!metrics) return <Html className="text-sm text-muted-foreground" html={t('qualityPanel.noMetrics')} />;

  const sharpnessWarn = metrics.sharpness < SHARPNESS_WARN_THRESHOLD;
  const overWarn = metrics.overexposed_pct > CLIPPING_WARN_PCT;
  const underWarn = metrics.underexposed_pct > CLIPPING_WARN_PCT;

  const rows = [
    { label: t('qualityPanel.metric.sharpness'), value: metrics.sharpness.toFixed(1), meter: <MeterBar value={metrics.sharpness} max={METER_MAX.sharpness} warn={sharpnessWarn} />, warn: sharpnessWarn ? t('qualityPanel.warn.sharpness') : null },
    { label: t('qualityPanel.metric.brightness'), value: metrics.brightness.toFixed(1), meter: <MeterBar value={metrics.brightness} max={METER_MAX.brightness} warn={false} /> },
    { label: t('qualityPanel.metric.contrast'), value: metrics.contrast.toFixed(1), meter: <MeterBar value={metrics.contrast} max={METER_MAX.contrast} warn={false} /> },
    { label: t('qualityPanel.metric.overexposed'), value: `${metrics.overexposed_pct.toFixed(1)}%`, meter: <MeterBar value={metrics.overexposed_pct} max={100} warn={overWarn} />, warn: overWarn ? t('qualityPanel.warn.overexposed') : null },
    { label: t('qualityPanel.metric.underexposed'), value: `${metrics.underexposed_pct.toFixed(1)}%`, meter: <MeterBar value={metrics.underexposed_pct} max={100} warn={underWarn} />, warn: underWarn ? t('qualityPanel.warn.underexposed') : null },
    { label: t('qualityPanel.metric.saturation'), value: metrics.saturation.toFixed(2), meter: <MeterBar value={metrics.saturation} max={METER_MAX.saturation} warn={false} /> },
    { label: t('qualityPanel.metric.colorfulness'), value: metrics.colorfulness.toFixed(2), meter: <MeterBar value={metrics.colorfulness} max={METER_MAX.colorfulness} warn={false} /> },
    { label: t('qualityPanel.metric.entropy'), value: metrics.entropy.toFixed(2), meter: <MeterBar value={metrics.entropy} max={METER_MAX.entropy} warn={false} /> },
    { label: t('qualityPanel.metric.averageColor'), value: `rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b})`, meter: <div className="w-6 h-6 rounded-full border shadow-inner" style={{ backgroundColor: `rgb(${metrics.average_r}, ${metrics.average_g}, ${metrics.average_b})` }} /> },
    { label: t('qualityPanel.metric.orientation'), value: metrics.orientation },
  ];

  return (
    <table className="w-full text-xs">
      <tbody className="divide-y">
        {rows.map((row, i) => (
          <tr key={i}>
            <td className="py-2 pr-2 text-muted-foreground">{row.label}</td>
            <td className="py-2 pr-2 font-mono">{row.value}</td>
            <td className="py-2 pr-2">{row.meter}</td>
            <td className="py-2 w-4">
              {row.warn && (
                <span title={row.warn}>
                  <AlertCircle className="w-4 h-4 text-destructive" />
                </span>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
