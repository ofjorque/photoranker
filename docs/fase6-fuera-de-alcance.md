# Fase 6 — Fuera de Alcance del MVP

> Documentar, no implementar. Ver `conventions.md` ("Definición de MVP") para el criterio de qué sí entra en el MVP.

- Soporte Mac/Linux. [Se mantiene pendiente]
- Detección de duplicados entre carpetas/viajes. [Liberar a otra fase]
- Acotar el pool de torneo por subcarpeta (`--scope=subfolder`) en vez de la carpeta raíz completa — el MVP siempre usa la carpeta raíz completa como pool único (ver `fase3-torneo.md`). [Liberar a otra fase]
- Lock manager propio para concurrencia GUI+CLI simultánea — el MVP asume un solo proceso escribiendo a la vez (SQLite en modo WAL, ver "Modelo de concurrencia" en `conventions.md`). [Liberar a otra fase]
- Vista en grilla (múltiples imágenes a la vez) en `variable-tag` — el MVP es exclusivamente modo "una por una" (ver `fase3-torneo.md`). [Liberar a otra fase]

## Siguiente fase

`fase7-mejoras-post-mvp.md` — a diferencia de esta fase, sí es implementable: mejoras de UX/GUI identificadas con la app en uso real que no contradicen ninguno de los no-objetivos de arriba.
