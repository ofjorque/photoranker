// Set mínimo de íconos SVG en línea — reemplaza la mezcla de emoji /
// símbolos Unicode / entidades HTML que convivía en distintos puntos de la
// app (main.ts, Lightbox.ts, QualityPanel.ts) sin ningún criterio común (ver
// reporte de exploración de la GUI). Estilo "outline" (stroke=currentColor,
// sin relleno) para que cada ícono herede el color de texto/acento del
// contexto vía CSS, sin necesitar una variante por tema.
function svg(inner: string): string {
  return `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;
}

export const icons = {
  folder: svg(
    '<path d="M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6Z"/>',
  ),
  burst: svg('<path d="M13 2 4 14h6l-1 8 9-12h-6l1-8Z"/>'),
  tag: svg(
    '<path d="M12.6 2H4a1 1 0 0 0-1 1v8.6a1 1 0 0 0 .3.7l9.4 9.4a1 1 0 0 0 1.4 0l7.6-7.6a1 1 0 0 0 0-1.4l-9.4-9.4a1 1 0 0 0-.7-.3Z"/><circle cx="7.5" cy="7.5" r="1.3"/>',
  ),
  cluster: svg(
    '<circle cx="6" cy="6" r="3"/><circle cx="18" cy="6" r="3"/><circle cx="12" cy="17" r="3.5"/><path d="M8.4 8 10 14M15.6 8 14 14"/>',
  ),
  trophy: svg(
    '<path d="M8 4h8v5a4 4 0 0 1-8 0V4Z"/><path d="M8 5H5a2 2 0 0 0 2 4M16 5h3a2 2 0 0 1-2 4"/><path d="M12 13v3M9 20h6M10 16h4v4h-4z"/>',
  ),
  export: svg('<path d="M12 3v12M7 8l5-5 5 5M5 21h14"/>'),
  settings: svg(
    '<circle cx="12" cy="12" r="3.2"/><path d="M19.4 13.5a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V20a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H4a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H10a1.7 1.7 0 0 0 1-1.5V4a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V10a1.7 1.7 0 0 0 1.5 1H20a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z"/>',
  ),
  close: svg('<path d="M6 6l12 12M18 6 6 18"/>'),
  warning: svg('<path d="M12 3 2 20h20L12 3Z"/><path d="M12 10v4M12 17.5h.01"/>'),
  chevronLeft: svg('<path d="M15 5 8 12l7 7"/>'),
  chevronRight: svg('<path d="M9 5l7 7-7 7"/>'),
  zoomIn: svg('<circle cx="10.5" cy="10.5" r="6.5"/><path d="M10.5 8v5M8 10.5h5M20 20l-4.3-4.3"/>'),
  zoomOut: svg('<circle cx="10.5" cy="10.5" r="6.5"/><path d="M8 10.5h5M20 20l-4.3-4.3"/>'),
  zoomReset: svg('<rect x="4" y="4" width="13" height="13" rx="2"/><path d="M20 9v11H9"/>'),
  check: svg('<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.5 2.5L16 9.5"/>'),
  trash: svg('<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>'),
} as const;

export type IconName = keyof typeof icons;
