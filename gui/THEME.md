# Dirección de diseño — PhotoRanker GUI

Ver checklist de `docs/fase5-gui.md`. Decisiones concretas, no "moderno" en abstracto:

- **Tipografía**: Inter Variable (`@fontsource-variable/inter`, bundleado — sin CDN, la app funciona offline). Pesos 400/500/600/700 vía el eje variable. Números tabulares (`font-variant-numeric: tabular-nums`) para `mu`/`sigma`/BIC, que se re-renderizan en vivo y no deben "bailar" de ancho.
- **Paleta**: la del tema embebido oscuro/claro exigido por `fase5-gui.md` (tokens exactos en `src/theme/tokens.css`). Acento violeta (`#7c6fff` oscuro / `#6952e0` claro) — frío, no compite con las miniaturas de fotos que dominan la pantalla.
- **Espaciado**: escala de 8px (`--spacing-unit`), múltiplos (4/8/16/24/32) — nunca un padding "al ojo".
- **Profundidad**: sombras suaves multicapa (`--shadow-sm/md/lg`) en vez de bordes de 1px; los paneles flotan sobre `--color-bg` con `--color-surface` + sombra, no con líneas divisorias.
- **Controles**: `<input>`/`<select>`/botones con estilo custom completo (radios `--radius-md`, sin apariencia nativa del navegador) — ver `src/theme/base.css`.
- **Elemento distintivo**: la transición de foco entre miniaturas del torneo — el borde de foco (`--color-focus-border`) se anima con `transition` + un halo (`box-shadow` difuso del mismo color) que crece/decrece al mover el foco, en vez de aparecer/desaparecer abruptamente. El badge de empate (`--color-tie-badge`) tiene un pulso sutil (`animation`) para diferenciarse a simple vista de un badge de posición asignada normal.

## Mecanismo de override (`theme_path`)

El tema embebido (`dark.css` o `light.css`, según `config.toml: theme`) se inyecta primero como un `<style>`; si `theme_path` apunta a un `.css` legible, su contenido se inyecta después, en un segundo `<style>` — así solo necesita redefinir las variables que le interesan. Fallback silencioso (sin romper la app) si el archivo no existe o no se puede leer — ver `src/theme/index.ts`.
