# 🏗️ Arquitectura, Principios y No-Objetivos

> Documento de referencia — consúltalo desde cualquier fase del roadmap. Ver también `database.md`, `conventions.md`.

## Descripción general

PhotoRanker es una suite de herramientas de escritorio (CLI + GUI) diseñada para fotógrafos que buscan organizar, clasificar y rankear grandes volúmenes de imágenes de manera lógica, matemática y sin fatiga de decisión.

Optimizado para flujos de trabajo en **RAW** (compatible con JPEG; **HEIC** solo en la medida en que traiga una miniatura JPEG embebida extraíble igual que un RAW — el toolchain del MVP, `image`/`imageproc`/`rawloader`, no decodifica HEIC completo, que requeriría `libheif` como dependencia adicional de sistema; si un HEIC no trae miniatura embebida legible, sigue el mismo camino que un RAW sin preview: `thumbnail_status='failed'`), PhotoRanker utiliza un núcleo escrito en **Rust** que se comunica con **R** para agrupar tus fotos mediante modelos de clases latentes (`clustMD`). Luego, las ordena de forma competitiva usando el algoritmo de emparejamiento estadístico **TrueSkill**, implementado mediante el crate **`skillratings`** (migrado desde Weng-Lin — ver "Torneos Jerárquicos" en `fase3-torneo.md` para el porqué). Todo esto manteniendo una integración 100% no destructiva compatible con **Darktable** mediante sincronización diferida con archivos `.xmp`.

La interacción principal es **100% navegable por teclado**: el usuario nunca necesita el mouse para completar un torneo.

---

## 🎯 Principios del Proyecto

- El usuario siempre toma la decisión final; el sistema sugiere y ordena, nunca decide por él.
- Nunca se modifica un RAW: todo el trabajo es no destructivo, solo se escriben sidecars `.xmp`.
- Todos los algoritmos son explicables: estadística clásica (`clustMD`, TrueSkill, percentiles), sin cajas negras.
- No se usan modelos de IA/Machine Learning para clasificar o decidir sobre las imágenes.
- Los resultados son deterministas dadas las mismas entradas y configuración (mismo `.photoranker.sqlite` + mismo `config.toml` → mismo resultado).
- Todo el estado persistente vive en SQLite y XMP — sin formatos propietarios ni bases de datos externas obligatorias.

## 🚫 No Objetivos

PhotoRanker **no** intenta:

- Revelar o editar archivos RAW.
- Reemplazar Darktable, Lightroom u otro gestor/editor.
- Editar píxeles de las fotografías (crop, color grading, retoque).
- Usar Machine Learning o reconocimiento facial para clasificar imágenes.
- Clasificar automáticamente el contenido semántico de la foto (eso lo decide el usuario vía variables personalizadas en `user_variables`, ver `database.md`).
- Sincronizar o subir fotos a la nube.

---

## 🏗️ Arquitectura: CLI-First + GUI

PhotoRanker está construido bajo la filosofía **CLI-First**. El verdadero "cerebro" de la aplicación es un ejecutable ultrarrápido en Rust (`photoranker.exe`) que puedes usar directamente desde la terminal. La interfaz gráfica (construida con **Tauri**) actúa simplemente como un panel de control visual que ejecuta estos comandos en segundo plano.

```text
[ Interfaz Gráfica (Tauri) ] ────(Ejecuta comandos)─────┐
                                                        │
[ Terminal de Usuario ] ─────────(Ejecuta comandos)─────┤
                                                        ▼
┌─────────────────────────────────────────────────────────────┐
│                   photoranker.exe (Rust CLI)                │
│  - Escaneo recursivo y extracción de miniaturas             │
│  - Hashing perceptual (pHash)                               │
│  - Métricas objetivas de calidad (nitidez, color, etc.)      │
│  - Motor del Torneo TrueSkill (crate skillratings)          │
│  - Lógica de lectura/escritura XMP (Batch)                  │
└──────┬───────────────────────────────┬───────────────┬─────┘
       │ (Lee/Escribe estado)          │ (Llama subproc.)│ (Upsert)
       ▼                                ▼                ▼
┌──────────────┐                 ┌──────────────┐  ┌─────────────────────┐
│ .photoranker │◄──(R lee/escribe)►│ Rscript.exe │  │ ~/.photoranker/      │
│ .sqlite      │                 │  (clustMD)   │  │ global_index.sqlite  │
│ (por carpeta)│                 └──────────────┘  │ (mu global, liviano) │
└──────────────┘                                    └─────────────────────┘
```

Cada carpeta de fotos tiene su propia base de datos SQLite (`.photoranker.sqlite`), que actúa como fuente de verdad local durante toda la sesión. El intercambio con R se realiza directamente sobre ese mismo archivo, sin pasos intermedios. Además, existe un **índice global liviano** (una sola base de datos en el directorio de configuración del usuario) que solo almacena `mu` por imagen de todas las carpetas procesadas, usado exclusivamente para calcular percentiles de estrellas de forma consistente entre sesiones — ver `database.md` para el esquema completo de ambas bases de datos.

## Ver también

- `database.md` — esquema SQL completo (BD local + índice global).
- `conventions.md` — crates oficiales, formato JSON, concurrencia, estilo de código, testing, MVP.
- `config.md` — `config.toml` completo documentado.
- `cli-reference.md` — catálogo de todos los comandos.
- `fase0-scaffolding.md` a `fase6-fuera-de-alcance.md` — roadmap por fases.
