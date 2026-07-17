# PhotoRanker GUI — Tauri + React + Tailwind + shadcn/ui

Envoltura visual de `core-cli/` (ver `docs/fase5-gui.md`) — Tauri 2, React 19,
Tailwind CSS y componentes de [shadcn/ui](https://ui.shadcn.com) (estilo
"new-york"). Ventana sin decoración nativa (`decorations: false` en
`src-tauri/tauri.conf.json`) con una barra de título propia
(`src/components/TitleBar.tsx`).

Migrado desde una versión anterior en TypeScript vanilla sin framework — ver
`docs/fase5-gui.md` para el detalle de la decisión y `THEME.md` para la
dirección de diseño (paleta de marca, tokens de theming, tema "glossy").

## Estructura

- `src/App.tsx` — shell: barra de título, nav lateral, enrutado por hash (`src/router.ts`).
- `src/views/*.tsx` — una vista por pantalla (Home, Bursts, Tournament, Cluster, Variables, Export, Settings).
- `src/components/*.tsx` — componentes compartidos; `src/components/ui/` son los primitivos generados por el CLI de shadcn (no editar a mano salvo necesidad real — `npx shadcn add <componente>` para agregar otros).
- `src/api/` — el puente hacia el CLI (`invoke('run_photoranker', ...)`), sin lógica de negocio — no cambia con el framework de UI.
- `src/i18n/` — diccionarios es/en (ver `docs/fase5-gui.md`, "Internacionalización").
- `src/theme/` — carga del tema embebido + override externo (`theme_path`, ver `docs/config.md`); los tokens en sí viven en `src/index.css`.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
