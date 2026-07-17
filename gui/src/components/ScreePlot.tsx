import { CartesianGrid, Line, LineChart, XAxis, YAxis } from 'recharts';
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart';
import { t } from '@/i18n';

export interface ScreePlotProps {
  bicByK: Record<string, number>;
}

const chartConfig = {
  bic: {
    label: 'BIC',
    color: 'hsl(var(--primary))',
  },
} satisfies ChartConfig;

/** Punto normal = `--primary`; el mejor k (mayor BIC, convención mclust) se
 *  resalta más grande en `--success` con su valor etiquetado arriba — mismo
 *  criterio visual que la versión anterior dibujada a mano en SVG. */
function BestKDot(props: { cx?: number; cy?: number; payload?: { k: number; bic: number }; bestK: number }) {
  const { cx, cy, payload, bestK } = props;
  if (cx === undefined || cy === undefined || !payload) return null;
  const isBest = payload.k === bestK;
  return (
    <g>
      <circle
        cx={cx}
        cy={cy}
        r={isBest ? 6 : 4}
        fill={isBest ? 'hsl(var(--success))' : 'hsl(var(--primary))'}
        stroke="hsl(var(--background))"
        strokeWidth={2}
      />
      {isBest && (
        <text x={cx} y={cy - 12} textAnchor="middle" fontSize={11} fontWeight={600} fill="hsl(var(--success))">
          {payload.bic.toFixed(1)}
        </text>
      )}
    </g>
  );
}

export function ScreePlot({ bicByK }: ScreePlotProps) {
  const data = Object.entries(bicByK)
    .map(([k, bic]) => ({ k: Number(k), bic }))
    .sort((a, b) => a.k - b.k);

  if (data.length === 0) {
    return <div className="p-8 text-center text-muted-foreground border rounded-md">{t('screePlot.noData')}</div>;
  }

  // Convención mclust: mayor BIC = mejor ajuste (no menor).
  const bestK = data.reduce((best, cur) => (cur.bic > best.bic ? cur : best)).k;

  return (
    <div className="w-full" role="img" aria-label={t('screePlot.ariaLabel')}>
      <ChartContainer config={chartConfig} className="aspect-auto h-[260px] w-full">
        <LineChart data={data} margin={{ top: 20, right: 20, left: 8, bottom: 8 }}>
          <CartesianGrid vertical={false} strokeOpacity={0.2} />
          <XAxis
            dataKey="k"
            tickLine={false}
            axisLine={false}
            label={{ value: t('screePlot.xAxis'), position: 'insideBottom', offset: -4, fontSize: 11 }}
          />
          <YAxis
            tickLine={false}
            axisLine={false}
            width={48}
            label={{ value: t('screePlot.yAxis'), angle: -90, position: 'insideLeft', fontSize: 11 }}
          />
          <ChartTooltip content={<ChartTooltipContent labelFormatter={(k) => `k = ${k}`} />} />
          <Line
            dataKey="bic"
            type="monotone"
            stroke="hsl(var(--primary))"
            strokeWidth={2.5}
            dot={(props) => <BestKDot key={props.payload?.k ?? props.cx} {...props} bestK={bestK} />}
          />
        </LineChart>
      </ChartContainer>
      <p
        className="mt-2 text-sm text-muted-foreground"
        dangerouslySetInnerHTML={{ __html: t('screePlot.caption', { bestK }) }}
      />
    </div>
  );
}
