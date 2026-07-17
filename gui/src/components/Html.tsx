// Algunas claves de i18n (ver i18n/es.ts/en.ts) llevan HTML simple
// (<code>, <kbd>, <strong>) heredado de cuando la GUI era vanilla TS con
// `innerHTML =`. `t()` devuelve texto plano para JSX, así que estas claves
// puntuales necesitan pasar por acá en vez de interpolarse directo — todo el
// contenido es un string constante escrito por nosotros (nunca datos del
// usuario más allá de valores ya sanitizados como nombres de carpeta), mismo
// perfil de riesgo que la versión anterior a esta migración.
export function Html({ html, className }: { html: string; className?: string }) {
  // eslint-disable-next-line react/no-danger
  return <span className={className} dangerouslySetInnerHTML={{ __html: html }} />;
}
