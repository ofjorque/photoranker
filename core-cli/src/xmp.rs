//! Sidecars `.xmp`: nombre de archivo (convención Darktable) y merge seguro
//! con `quick-xml` — ver docs/fase4-exportacion.md, "Nombre del archivo
//! sidecar" y "Política de escritura: merge seguro, no sobrescritura total".

use crate::error::{AppError, AppResult};
use quick_xml::escape::escape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;
use std::path::{Path, PathBuf};

const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";

/// Convención Darktable: el sidecar es el **nombre completo del archivo
/// original, incluyendo su extensión**, más `.xmp`, en la misma carpeta —
/// nunca la convención Adobe/Lightroom (`IMG_1234.xmp`, sin la extensión
/// original), que Darktable no reconoce. Única función que codifica esta
/// regla (ver fase4-exportacion.md), usada por todo el módulo de exportación.
pub fn xmp_sidecar_path(original: &Path) -> PathBuf {
    let mut name = original.as_os_str().to_os_string();
    name.push(".xmp");
    PathBuf::from(name)
}

/// Escribe (crea o fusiona) el sidecar `.xmp` de `original` con las estrellas
/// y tags de cluster dados. Ver "Política de escritura" en
/// fase4-exportacion.md: si ya existe un `.xmp`, se preserva todo lo que
/// PhotoRanker no gestiona y solo se actualiza/inyecta `xmp:Rating` y los
/// `<rdf:li>` de `dc:subject`.
pub fn write_sidecar(original: &Path, rating: i32, subject_tags: &[String]) -> AppResult<()> {
    let sidecar_path = xmp_sidecar_path(original);
    let xml = match std::fs::read_to_string(&sidecar_path) {
        Ok(existing) => merge_xmp(&existing, rating, subject_tags)?,
        Err(_) => build_new_xmp(rating, subject_tags),
    };
    std::fs::write(&sidecar_path, xml)?;
    Ok(())
}

fn build_new_xmp(rating: i32, subject_tags: &[String]) -> String {
    let mut subject = String::new();
    if !subject_tags.is_empty() {
        subject.push_str("   <dc:subject>\n    <rdf:Bag>\n");
        for tag in subject_tags {
            subject.push_str(&format!("     <rdf:li>{}</rdf:li>\n", escape(tag)));
        }
        subject.push_str("    </rdf:Bag>\n   </dc:subject>\n");
    }

    if subject.is_empty() {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:xmp=\"{NS_XMP}\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmp:Rating=\"{rating}\"/>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n"
        )
    } else {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:xmp=\"{NS_XMP}\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmp:Rating=\"{rating}\">\n\
{subject}\
  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n"
        )
    }
}

fn local_name(qname: quick_xml::name::QName) -> Vec<u8> {
    qname.local_name().as_ref().to_vec()
}

/// Reconstruye el atributo `xmp:Rating` de un `rdf:Description` existente,
/// preservando todos los demás atributos (y namespaces) tal cual, agregando
/// `xmlns:xmp` solo si todavía no estaba declarado.
fn with_rating_attr(start: &BytesStart<'_>, rating: i32) -> AppResult<BytesStart<'static>> {
    let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let mut new_start = BytesStart::new(name);
    let mut has_xmp_ns = false;
    let mut set_rating = false;

    for attr in start.attributes() {
        let attr = attr.map_err(|e| {
            AppError::XmpParseError(format!("atributo de rdf:Description inválido: {e}"))
        })?;
        let key = attr.key.as_ref().to_vec();
        if key == b"xmlns:xmp" {
            has_xmp_ns = true;
        }
        if key == b"xmp:Rating" {
            set_rating = true;
            new_start.push_attribute(("xmp:Rating", rating.to_string().as_str()));
            continue;
        }
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| AppError::XmpParseError(format!("valor de atributo inválido: {e}")))?
            .into_owned();
        let key_str = String::from_utf8_lossy(&key).into_owned();
        new_start.push_attribute((key_str.as_str(), value.as_str()));
    }

    if !set_rating {
        new_start.push_attribute(("xmp:Rating", rating.to_string().as_str()));
    }
    if !has_xmp_ns {
        new_start.push_attribute(("xmlns:xmp", NS_XMP));
    }

    Ok(new_start)
}

/// Eventos para un bloque `<dc:subject><rdf:Bag>...<rdf:li>tag</rdf:li>...</rdf:Bag></dc:subject>`.
fn subject_block_events(tags: &[String]) -> Vec<Event<'static>> {
    let mut events = vec![
        Event::Start(BytesStart::new("dc:subject")),
        Event::Start(BytesStart::new("rdf:Bag")),
    ];
    for tag in tags {
        events.push(Event::Start(BytesStart::new("rdf:li")));
        events.push(Event::Text(BytesText::from_escaped(
            escape(tag).into_owned(),
        )));
        events.push(Event::End(BytesEnd::new("rdf:li")));
    }
    events.push(Event::End(BytesEnd::new("rdf:Bag")));
    events.push(Event::End(BytesEnd::new("dc:subject")));
    events
}

/// Solo los `<rdf:li>` que faltan (los tags ya presentes no se tocan ni se
/// duplican).
fn li_events_for_missing(existing_tags: &[String], wanted: &[String]) -> Vec<Event<'static>> {
    wanted
        .iter()
        .filter(|tag| !existing_tags.iter().any(|e| e == *tag))
        .flat_map(|tag| {
            vec![
                Event::Start(BytesStart::new("rdf:li")),
                Event::Text(BytesText::from_escaped(escape(tag).into_owned())),
                Event::End(BytesEnd::new("rdf:li")),
            ]
        })
        .collect()
}

/// Fusiona un `.xmp` existente: preserva íntegramente cualquier namespace o
/// etiqueta que PhotoRanker no gestiona, y solo actualiza/inyecta
/// `xmp:Rating` y agrega (sin borrar) los `<rdf:li>` de `dc:subject` que
/// falten (ver fase4-exportacion.md, "Política de escritura"). **Simplificación
/// de MVP**: solo se modifica el primer `rdf:Description` del archivo — el
/// caso típico (un único bloque `Description` con todos los namespaces, como
/// el que genera Darktable/nuestra propia plantilla); varios `rdf:Description`
/// hermanos repartiendo namespaces (patrón menos común, también válido en XMP)
/// no está cubierto y debe revisarse con el usuario si aparece en la práctica.
pub fn merge_xmp(existing: &str, rating: i32, subject_tags: &[String]) -> AppResult<String> {
    let mut reader = Reader::from_str(existing);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut description_handled = false;
    let mut description_depth: Option<i32> = None;
    let mut depth = 0i32;
    let mut subject_seen = false;
    let mut in_subject = false;
    let mut in_bag = false;
    let mut capturing_li = false;
    let mut existing_tags: Vec<String> = Vec::new();
    let mut li_buf = String::new();

    loop {
        let event = reader
            .read_event()
            .map_err(|e| AppError::XmpParseError(format!("XMP existente inválido: {e}")))?;

        match event {
            Event::Eof => break,

            Event::Start(start) => {
                let candidate_depth = depth + 1;
                let local = local_name(start.name());

                if !description_handled && local == b"Description" {
                    description_handled = true;
                    description_depth = Some(candidate_depth);
                    let modified = with_rating_attr(&start, rating)?;
                    writer
                        .write_event(Event::Start(modified))
                        .map_err(AppError::from)?;
                } else {
                    if description_depth == Some(depth) && !subject_seen && local == b"subject" {
                        subject_seen = true;
                        in_subject = true;
                    } else if in_subject && local == b"Bag" {
                        in_bag = true;
                    } else if in_bag && local == b"li" {
                        capturing_li = true;
                        li_buf.clear();
                    }
                    writer
                        .write_event(Event::Start(start))
                        .map_err(AppError::from)?;
                }
                depth = candidate_depth;
            }

            Event::Text(text) => {
                if capturing_li {
                    let decoded = text
                        .decode()
                        .map_err(|e| AppError::XmpParseError(format!("texto inválido: {e}")))?;
                    li_buf.push_str(&decoded);
                }
                writer
                    .write_event(Event::Text(text))
                    .map_err(AppError::from)?;
            }

            Event::End(end) => {
                let local = local_name(end.name());

                if capturing_li && local == b"li" {
                    existing_tags.push(li_buf.clone());
                    capturing_li = false;
                }

                if in_bag && local == b"Bag" {
                    for extra in li_events_for_missing(&existing_tags, subject_tags) {
                        writer.write_event(extra).map_err(AppError::from)?;
                    }
                    in_bag = false;
                }

                if in_subject && local == b"subject" {
                    in_subject = false;
                }

                let closing_description =
                    description_depth == Some(depth) && local == b"Description";
                if closing_description && !subject_seen && !subject_tags.is_empty() {
                    for extra in subject_block_events(subject_tags) {
                        writer.write_event(extra).map_err(AppError::from)?;
                    }
                }
                if closing_description {
                    description_depth = None;
                }

                writer
                    .write_event(Event::End(end))
                    .map_err(AppError::from)?;
                depth -= 1;
            }

            Event::Empty(start) => {
                let local = local_name(start.name());

                if !description_handled && local == b"Description" {
                    description_handled = true;
                    let modified = with_rating_attr(&start, rating)?;
                    if subject_tags.is_empty() {
                        writer
                            .write_event(Event::Empty(modified))
                            .map_err(AppError::from)?;
                    } else {
                        writer
                            .write_event(Event::Start(modified.to_owned()))
                            .map_err(AppError::from)?;
                        for extra in subject_block_events(subject_tags) {
                            writer.write_event(extra).map_err(AppError::from)?;
                        }
                        let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                        writer
                            .write_event(Event::End(BytesEnd::new(name)))
                            .map_err(AppError::from)?;
                    }
                } else if description_depth == Some(depth) && !subject_seen && local == b"subject" {
                    subject_seen = true;
                    if subject_tags.is_empty() {
                        writer
                            .write_event(Event::Empty(start))
                            .map_err(AppError::from)?;
                    } else {
                        writer
                            .write_event(Event::Start(BytesStart::new("dc:subject")))
                            .map_err(AppError::from)?;
                        writer
                            .write_event(Event::Start(BytesStart::new("rdf:Bag")))
                            .map_err(AppError::from)?;
                        for extra in li_events_for_missing(&[], subject_tags) {
                            writer.write_event(extra).map_err(AppError::from)?;
                        }
                        writer
                            .write_event(Event::End(BytesEnd::new("rdf:Bag")))
                            .map_err(AppError::from)?;
                        writer
                            .write_event(Event::End(BytesEnd::new("dc:subject")))
                            .map_err(AppError::from)?;
                    }
                } else if in_subject && local == b"Bag" {
                    if subject_tags.is_empty() {
                        writer
                            .write_event(Event::Empty(start))
                            .map_err(AppError::from)?;
                    } else {
                        writer
                            .write_event(Event::Start(BytesStart::new("rdf:Bag")))
                            .map_err(AppError::from)?;
                        for extra in li_events_for_missing(&[], subject_tags) {
                            writer.write_event(extra).map_err(AppError::from)?;
                        }
                        writer
                            .write_event(Event::End(BytesEnd::new("rdf:Bag")))
                            .map_err(AppError::from)?;
                    }
                } else {
                    writer
                        .write_event(Event::Empty(start))
                        .map_err(AppError::from)?;
                }
            }

            other => {
                writer.write_event(other).map_err(AppError::from)?;
            }
        }
    }

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes)
        .map_err(|e| AppError::XmpParseError(format!("salida XMP no es UTF-8 válido: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_path_uses_darktable_convention_full_name_plus_xmp() {
        let original = Path::new("C:/Fotos/Boda/IMG_1234.CR2");
        let sidecar = xmp_sidecar_path(original);
        assert_eq!(sidecar, Path::new("C:/Fotos/Boda/IMG_1234.CR2.xmp"));
    }

    #[test]
    fn new_xmp_contains_rating_and_subject_tag() {
        let xml = build_new_xmp(4, &["Retratos nocturnos".to_string()]);
        assert!(xml.contains("xmp:Rating=\"4\""));
        assert!(xml.contains("<rdf:li>Retratos nocturnos</rdf:li>"));
    }

    #[test]
    fn new_xmp_without_tags_has_no_subject_block() {
        let xml = build_new_xmp(-1, &[]);
        assert!(xml.contains("xmp:Rating=\"-1\""));
        assert!(!xml.contains("dc:subject"));
    }

    #[test]
    fn merge_preserves_unmanaged_namespace_and_updates_rating() {
        let existing = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:lr="http://ns.adobe.com/lightroom/1.0/"
    xmp:Rating="2"
    xmp:Label="Green"
    lr:hierarchicalSubject="Viajes|Boda">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Viaje 2024</rdf:li>
    </rdf:Bag>
   </dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;
        let merged = merge_xmp(existing, 5, &["Retratos nocturnos".to_string()]).unwrap();
        assert!(merged.contains("xmp:Rating=\"5\""));
        assert!(merged.contains("xmp:Label=\"Green\""));
        assert!(merged.contains("lr:hierarchicalSubject=\"Viajes|Boda\""));
        assert!(merged.contains("<rdf:li>Viaje 2024</rdf:li>"));
        assert!(merged.contains("<rdf:li>Retratos nocturnos</rdf:li>"));
    }

    #[test]
    fn merge_does_not_duplicate_tag_already_present() {
        let existing = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmp:Rating="3">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Retratos nocturnos</rdf:li>
    </rdf:Bag>
   </dc:subject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;
        let merged = merge_xmp(existing, 3, &["Retratos nocturnos".to_string()]).unwrap();
        let count = merged
            .matches("<rdf:li>Retratos nocturnos</rdf:li>")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn merge_injects_subject_block_when_missing_entirely() {
        let existing = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="1"
    xmp:Label="Red">
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;
        let merged = merge_xmp(existing, 1, &["Retratos nocturnos".to_string()]).unwrap();
        assert!(merged.contains("<rdf:li>Retratos nocturnos</rdf:li>"));
        assert!(merged.contains("xmp:Label=\"Red\""));
    }

    #[test]
    fn merge_handles_self_closing_description_without_tags() {
        let existing = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="2"/>
 </rdf:RDF>
</x:xmpmeta>
"#;
        let merged = merge_xmp(existing, -1, &[]).unwrap();
        assert!(merged.contains("xmp:Rating=\"-1\""));
    }
}
