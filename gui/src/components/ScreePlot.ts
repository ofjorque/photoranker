// Scree plot de BIC para `cluster --preview` (ver docs/fase2-clustering.md:
// "mayor valor = mejor ajuste", convención mclust). SVG inline, sin
// dependencias externas de gráficos, leyendo solo var(--...) para color.
import { t } from '../i18n';

export function renderScreePlot(container: HTMLElement, bicByK: Record<string, number>): void {
  container.innerHTML = '';
  const entries = Object.entries(bicByK)
    .map(([k, bic]) => [Number(k), bic] as [number, number])
    .sort((a, b) => a[0] - b[0]);

  if (entries.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'empty-state';
    empty.textContent = t('screePlot.noData');
    container.appendChild(empty);
    return;
  }

  const width = 640;
  const height = 260;
  const padding = { top: 20, right: 20, bottom: 36, left: 56 };
  const plotW = width - padding.left - padding.right;
  const plotH = height - padding.top - padding.bottom;

  const ks = entries.map(([k]) => k);
  const bics = entries.map(([, b]) => b);
  const minBic = Math.min(...bics);
  const maxBic = Math.max(...bics);
  const bicRange = maxBic - minBic || 1;
  const bestK = entries.reduce((best, cur) => (cur[1] > best[1] ? cur : best))[0];

  const xFor = (k: number) => {
    const minK = Math.min(...ks);
    const maxK = Math.max(...ks);
    const range = maxK - minK || 1;
    return padding.left + ((k - minK) / range) * plotW;
  };
  const yFor = (bic: number) => padding.top + plotH - ((bic - minBic) / bicRange) * plotH;

  const points = entries.map(([k, bic]) => `${xFor(k)},${yFor(bic)}`).join(' ');

  const svgNs = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(svgNs, 'svg');
  svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  svg.setAttribute('width', '100%');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', t('screePlot.ariaLabel'));

  const axisColor = getComputedStyle(container).getPropertyValue('--color-border') || '#2c2c3a';
  const mutedColor =
    getComputedStyle(container).getPropertyValue('--color-text-muted') || '#9a9aa5';
  const accentColor = getComputedStyle(container).getPropertyValue('--color-accent') || '#7c6fff';
  const successColor =
    getComputedStyle(container).getPropertyValue('--color-success') || '#3ecf8e';

  // Ejes
  const axisGroup = document.createElementNS(svgNs, 'g');
  const yAxis = document.createElementNS(svgNs, 'line');
  yAxis.setAttribute('x1', String(padding.left));
  yAxis.setAttribute('y1', String(padding.top));
  yAxis.setAttribute('x2', String(padding.left));
  yAxis.setAttribute('y2', String(padding.top + plotH));
  yAxis.setAttribute('stroke', axisColor);
  const xAxis = document.createElementNS(svgNs, 'line');
  xAxis.setAttribute('x1', String(padding.left));
  xAxis.setAttribute('y1', String(padding.top + plotH));
  xAxis.setAttribute('x2', String(padding.left + plotW));
  xAxis.setAttribute('y2', String(padding.top + plotH));
  xAxis.setAttribute('stroke', axisColor);
  axisGroup.appendChild(yAxis);
  axisGroup.appendChild(xAxis);
  svg.appendChild(axisGroup);

  // Línea
  const polyline = document.createElementNS(svgNs, 'polyline');
  polyline.setAttribute('points', points);
  polyline.setAttribute('fill', 'none');
  polyline.setAttribute('stroke', accentColor);
  polyline.setAttribute('stroke-width', '2.5');
  polyline.setAttribute('stroke-linejoin', 'round');
  polyline.setAttribute('stroke-linecap', 'round');
  svg.appendChild(polyline);

  // Puntos + labels
  entries.forEach(([k, bic]) => {
    const cx = xFor(k);
    const cy = yFor(bic);
    const isBest = k === bestK;

    const circle = document.createElementNS(svgNs, 'circle');
    circle.setAttribute('cx', String(cx));
    circle.setAttribute('cy', String(cy));
    circle.setAttribute('r', isBest ? '6' : '4');
    circle.setAttribute('fill', isBest ? successColor : accentColor);
    circle.setAttribute('stroke', 'var(--color-surface)');
    circle.setAttribute('stroke-width', '2');
    svg.appendChild(circle);

    const label = document.createElementNS(svgNs, 'text');
    label.setAttribute('x', String(cx));
    label.setAttribute('y', String(padding.top + plotH + 18));
    label.setAttribute('text-anchor', 'middle');
    label.setAttribute('font-size', '11');
    label.setAttribute('fill', mutedColor);
    label.textContent = String(k);
    svg.appendChild(label);

    if (isBest) {
      const bestLabel = document.createElementNS(svgNs, 'text');
      bestLabel.setAttribute('x', String(cx));
      bestLabel.setAttribute('y', String(cy - 12));
      bestLabel.setAttribute('text-anchor', 'middle');
      bestLabel.setAttribute('font-size', '11');
      bestLabel.setAttribute('font-weight', '600');
      bestLabel.setAttribute('fill', successColor);
      bestLabel.textContent = bic.toFixed(1);
      svg.appendChild(bestLabel);
    }
  });

  const xAxisTitle = document.createElementNS(svgNs, 'text');
  xAxisTitle.setAttribute('x', String(padding.left + plotW / 2));
  xAxisTitle.setAttribute('y', String(height - 4));
  xAxisTitle.setAttribute('text-anchor', 'middle');
  xAxisTitle.setAttribute('font-size', '11');
  xAxisTitle.setAttribute('fill', mutedColor);
  xAxisTitle.textContent = t('screePlot.xAxis');
  svg.appendChild(xAxisTitle);

  const yAxisTitle = document.createElementNS(svgNs, 'text');
  yAxisTitle.setAttribute('x', '-' + (padding.top + plotH / 2));
  yAxisTitle.setAttribute('y', '14');
  yAxisTitle.setAttribute('text-anchor', 'middle');
  yAxisTitle.setAttribute('transform', 'rotate(-90)');
  yAxisTitle.setAttribute('font-size', '11');
  yAxisTitle.setAttribute('fill', mutedColor);
  yAxisTitle.textContent = t('screePlot.yAxis');
  svg.appendChild(yAxisTitle);

  container.appendChild(svg);

  const caption = document.createElement('p');
  caption.style.marginTop = '8px';
  caption.innerHTML = t('screePlot.caption', { bestK });
  container.appendChild(caption);
}
