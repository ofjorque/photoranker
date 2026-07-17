# Fase 5 — GUI (Tauri)

> Ver también: `conventions.md` (API interna GUI↔CLI), `fase3-torneo.md` (mecánica de teclado a replicar visualmente), `fase1-ingesta.md` (métricas de calidad a mostrar), `fase2-clustering.md` (scree plot de BIC), `config.md` (`theme`, `theme_path`).

La GUI es la envoltura visual de todo lo implementado en las fases 0-4 — no agrega lógica nueva (ver "Definición de MVP" en `conventions.md`). Invoca `photoranker.exe` como subproceso según el contrato de "API interna" de `conventions.md`.

## Identidad visual: no es la interfaz por defecto de Tauri/webview

La GUI debe sentirse diseñada, no un formulario HTML sin estilizar. Esto no es negociable como "detalle estético" — es parte del alcance de la Fase 5, con los mismos criterios de precisión que el resto del spec:

- **Dirección de diseño concreta, no "moderno" en abstracto**: tipografía con carácter (ej. Inter, Geist — no `system-ui`/Arial por defecto), una escala de espaciado consistente (no valores de padding/margin al azar), controles de formulario con estilo custom (nunca el look nativo de `<input>`/`<select>` del navegador), profundidad sutil (sombras suaves en vez de bordes duros de 1px), y al menos un elemento distintivo que el ojo recuerde (ej. la transición de foco entre miniaturas, o el estilo de los badges de empate de `fase3-torneo.md`).
- Quien implemente esta fase debe **proponer** una dirección de diseño concreta (tipografía + paleta + un acento) antes de escribir componentes, no ir directo a código con estilos por defecto.

## Theming: design tokens vía CSS custom properties

**Decisión de diseño**: toda la interfaz se estiliza con **variables CSS (`:root { --color-accent: ...; }`)**, nunca valores de color/tipografía hardcodeados directamente en los componentes. Cada componente lee `var(--color-accent)`, `var(--spacing-unit)`, etc. — nunca un literal. Esto es lo que permite personalización externa sin tocar el código de componentes, siguiendo el mismo patrón que temas de VS Code u otras apps personalizables.

**Tokens mínimos que debe exponer el tema embebido por defecto** (nombres exactos, para que un override de usuario sepa qué variables puede tocar):

```css
:root {
  --color-bg: #0f0f14;
  --color-surface: #1a1a24;
  --color-accent: #7c6fff;
  --color-text: #e8e8ec;
  --color-text-muted: #9a9aa5;
  --color-tie-badge: #f0a030;      /* badge de empate, ver fase3-torneo.md */
  --color-focus-border: #4a90ff;   /* borde de foco, ver fase3-torneo.md */
  --color-success-text: #04140c;   /* texto sobre --color-success (badge de posición) */
  --color-tie-badge-text: #221200; /* texto sobre --color-tie-badge */
  --font-family: 'Inter', sans-serif;
  --radius-md: 8px;
  --spacing-unit: 8px;
}
```

`--color-success-text`/`--color-tie-badge-text` existen porque el contraste correcto para el texto de un badge depende del brillo del fondo (`--color-success`/`--color-tie-badge`), que cambia entre tema oscuro y claro — no se puede derivar en CSS puro, así que son tokens independientes por tema en vez de un valor fijo hardcodeado en el componente.

**Mecanismo de override por el usuario**:
- `config.toml` define `theme_path` (ver `config.md`) — ruta opcional a un archivo `.css` externo (ej. `~/.photoranker/theme.css`).
- Al arrancar, la GUI inyecta el CSS embebido por defecto (`theme = "dark"`/`"light"` de `config.toml`) y, si `theme_path` apunta a un archivo existente, inyecta ese archivo **después**, como un `<style>` adicional — así el override solo necesita redefinir las variables que le interesan (ej. solo `--color-accent`), y el resto hereda del tema base sin que el usuario tenga que declarar cada token.
- Si `theme_path` no existe o el archivo no se puede leer, se ignora silenciosamente y se usa solo el tema embebido (no es un error bloqueante — un CSS de usuario mal formado no debe romper la app).
- No se valida el contenido del CSS del usuario más allá de que el archivo exista y sea legible; si el usuario define una variable con un valor inválido, es su responsabilidad — no se sobre-ingenieriza un validador de CSS para el MVP.

### Dirección "glossy" del tema oscuro (extensión de tokens, agregado por feedback de uso real)

El tema oscuro por defecto se refuerza con una estética "glossy"/vidrio, pedida explícitamente por el usuario — el tema claro se mantiene deliberadamente simple/plano por contraste, ver `THEME.md`. Extiende (no reemplaza) los tokens de arriba:

```css
:root {
  /* Stack de sombras con highlight superior sutil — mismos nombres que ya
     leía todo componente, solo cambian los valores en dark.css. */
  --shadow-sm: /* ... con inset 0 1px 0 rgba(255,255,255,0.04) además de la sombra de caída */;
  --shadow-md: /* ídem, capas más profundas */;
  --shadow-lg: /* ídem */;
  --focus-halo: /* halo de foco existente + una capa extra de glow difuso */;
  --shadow-glow-accent: 0 0 32px 8px rgba(124, 111, 255, 0.35); /* glow ancho opcional, se suma a --focus-halo en el foco del torneo */
  --color-tie-badge-rgb: 240, 160, 48; /* triplete RGB de --color-tie-badge, para el glow del pulso de empate */
  --gradient-surface: linear-gradient(155deg, ...);         /* reemplaza el fill plano de .panel */
  --gradient-surface-raised: linear-gradient(155deg, ...);  /* reemplaza el fill plano de .panel-nested */
  --gradient-ambient: radial-gradient(circle at 30% -10%, rgba(124, 111, 255, 0.16), transparent 60%); /* glow tenue detrás de todo el shell (body) */
  --glass-bg: rgba(26, 26, 36, 0.72);   /* fondo translúcido para overlays que flotan sobre fotos */
  --glass-blur: 16px;                    /* backdrop-filter de esos mismos overlays */
}
```

- `--gradient-surface`/`--gradient-surface-raised` sustituyen el fill plano de `--color-surface`/`--color-surface-raised` en `.panel`/`.panel-nested` (`base.css`) — los paneles normales, que están sobre el fondo plano de la app y no sobre fotos, usan este gradiente opaco.
- `--glass-bg`/`--glass-blur` (con `backdrop-filter`) se usan solo en los overlays que sí flotan sobre contenido de fotos: `ConfirmDialog`, `Lightbox`, `LoadingOverlay`.
- El tema claro (`light.css`) define los **mismos nombres de token** (obligatorio — si no, las reglas de `base.css`/componentes quedan rotas al cambiar de tema) pero con valores casi imperceptibles (gradientes de 1-2 tonos, sin el highlight `inset` en las sombras, glow ambiental muy tenue) — es una decisión de diseño explícita, no un descuido: el pedido de "glossy" fue específicamente para oscuro.

## Internacionalización (i18n)

**Decisión de diseño**: la GUI soporta español e inglés mediante dos diccionarios planos, sin librería externa (consistente con el resto de la Fase 5: vanilla TS, sin dependencias más allá de lo estrictamente necesario). Todo el copy de `gui/src/**` (vistas y componentes) pasa por una función de lookup — nunca un string en español o inglés hardcodeado directamente en un template. Esto **no** aplica al texto que la GUI parsea desde stdout/stderr del propio `photoranker.exe` (logs de `tracing`, siempre en español) — traducir la salida del CLI está fuera de este alcance.

- **Diccionarios**: `gui/src/i18n/es.ts` y `gui/src/i18n/en.ts`, cada uno un `Record<string, string>` plano con el mismo conjunto de claves. Convención de nombres: `<vista_o_componente>.<área>.<elemento>` (ej. `home.init.scanning`, `tournament.hint.moveFocus`), más un namespace `common.*` para strings reusados entre vistas (botones como "Guardar"/"Cancelar").
- **Lookup**: `gui/src/i18n/index.ts` expone `t(key: string, vars?: Record<string, string | number>): string`. Interpolación simple de `{varName}` sobre el string ya resuelto. Si la clave no existe en el idioma activo, cae a español; si tampoco existe ahí, devuelve `[[clave]]` y emite un `console.warn` — nunca rompe el render por una clave faltante.
- **Persistencia**: `config.toml` define `language` (ver `config.md`, `"es"`/`"en"`, default `"es"`) — mismo mecanismo de comandos Tauri dedicados que `theme`/`theme_path` (`read_language_config`/`write_language_config`, cada uno toca solo esa clave del TOML sin tocar el resto).
- **Selector**: vive en la pantalla de Ajustes, junto al selector de tema — cambiarlo aplica al instante (re-renderiza la vista actual con el nuevo diccionario) y persiste al guardar.
- `document.documentElement.lang` se mantiene sincronizado con el idioma activo en todo momento (accesibilidad).

## Acceso a miniaturas y métricas de calidad desde la GUI

Ningún comando de las fases 1-4 exponía los bytes de `images.thumbnail` ni las filas de `image_quality_metrics` por stdout — solo el modo TUI (`variable-tag`) los leía en memoria, directamente sobre la conexión SQLite del propio proceso. Tampoco existía forma de listar los bursts `pending` generados por `burst-detect` (solo se devolvían contadores agregados). Como `conventions.md` prohíbe que la GUI lea `.photoranker.sqlite` directamente, esta fase agrega tres comandos de solo lectura (sin backup, no tocan `mu`/`sigma`/`rejected`/`cluster_id`) para cerrar ese hueco, siguiendo el mismo sobre JSON del resto del CLI:

- `photoranker get-thumbnail --image-id <id>`: `data = {"id": <id>, "thumbnail_b64": "<JPEG en base64>"}`. Devuelve `THUMBNAIL_FAILED` si `thumbnail_status='failed'` (no hay bytes que codificar) e `IMAGE_NOT_FOUND` si el id no existe.
- `photoranker get-quality-metrics --image-id <id>`: `data = {"id": <id>, "metrics": {...} | null}` (con las columnas de `image_quality_metrics`; `null` si la imagen no tiene fila, ej. si su miniatura falló). `IMAGE_NOT_FOUND` si el id no existe.
- `photoranker list-bursts`: `data = [{"id": <burst_id>, "images": [{"id": <image_id>, "file_path": "..."}, ...]}, ...]` — solo bursts con `status='pending'` (los `completed` ya fueron resueltos y no vuelven a aparecer), cada uno con sus imágenes miembro ya resueltas por JOIN, para que la GUI arme el minitorneo sin una llamada extra por burst.

Dos comandos más, agregados tras una segunda ronda de feedback probando la GUI contra una biblioteca real:

- `photoranker list-clusters`: `data = [{"id": <cluster_id>, "name": "..."|null, "member_count": <n>, "representative_images": [{"id": <image_id>, "file_path": "...", "probability": <f64>}, ...(hasta 4, orden probability desc)]}, ...]` — para que la GUI muestre unas pocas fotos representativas de cada cluster (las de mayor `probability`, argmax de pertenencia) antes de que el usuario elija el nombre con `cluster-rename`.
- `photoranker get-variable-values --variable <nombre>`: `data = [{"id": <image_id>, "file_path": "...", "value": <f64>|null}, ...]` (solo imágenes activas, `rejected=0 AND missing=0`) — para que la GUI pueda mostrar/editar el valor de una variable foto por foto (clasificación visual) sin adivinar cuáles ya están etiquetadas. Antes de este comando, `image_variable_values` solo se podía leer desde el modo TUI `variable-tag`.

La GUI llama a `get-thumbnail` por cada imagen visible en pantalla (torneo, ráfagas, panel de referencia) y decodifica el base64 a un `<img src="data:image/jpeg;base64,...">` o equivalente — nunca abre el `.sqlite` por su cuenta.

## Checklist de implementación

- [ ] Proponer y documentar (como comentario en el CSS base o un `THEME.md` corto) una dirección de diseño concreta: tipografía, paleta base, y el elemento distintivo elegido.
- [ ] Implementar el sistema de variables CSS con los tokens mínimos listados arriba; ningún componente debe usar colores/tipografía hardcodeados.
- [ ] Implementar la carga del tema embebido (`dark`/`light` según `config.toml`) + inyección opcional de `theme_path` como override, con fallback silencioso si el archivo no existe o es inválido.
- [ ] Envolver todos los comandos anteriores como llamadas de subproceso desde Tauri.
- [ ] Implementar navegación e interacción por teclado (flechas/Tab para foco, `1`–`5` para asignar posición, `Enter` para confirmar con validación de completitud, `Backspace`/`R` para reset) — replicando exactamente la mecánica descrita en `fase3-torneo.md`.
- [ ] Feedback visual: foco = borde azul (`var(--color-focus-border)`); posición asignada = badge verde con número; empate = badge naranjo con ícono "=" (`var(--color-tie-badge)`).
- [ ] Scree plot de BIC para `cluster --preview` (ver `fase2-clustering.md`).
- [ ] Panel de referencia con métricas objetivas de calidad por imagen (valores e íconos de advertencia para baja nitidez o clipping, ver `fase1-ingesta.md`).

## Siguiente fase

`fase6-fuera-de-alcance.md` (no implementar; documentar los límites del MVP)
