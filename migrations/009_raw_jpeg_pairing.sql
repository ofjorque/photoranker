-- Ver docs/fase1-ingesta.md, "RAW + JPEG del mismo disparo cuentan como 1 sola
-- foto" (agregado en Fase 5 por feedback de uso real). `init` empareja, dentro
-- de la misma corrida, un archivo RAW y un JPEG que comparten carpeta y nombre
-- base (sin extensión, case-insensitive) y que todavía no existen en `images`
-- — un solo registro por par, con `file_path` apuntando al RAW (el archivo
-- "maestro") y `paired_path` al JPEG (usado como fuente de miniatura/pHash/
-- métricas, más confiable que decodificar el RAW). `export-xmp` escribe un
-- sidecar .xmp para cada uno de los dos archivos, con el mismo rating/label/
-- cluster. NULL si la imagen no tiene par (caso normal, un solo archivo).
ALTER TABLE images ADD COLUMN paired_path TEXT;
