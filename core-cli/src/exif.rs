//! Lectura de metadatos EXIF (crate `kamadak-exif`, ver docs/database.md,
//! sección "Estructura de `exif_json`").

use serde::Serialize;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExifData {
    pub iso: Option<u32>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<f64>,
    pub focal_length: Option<f64>,
    pub lens: Option<String>,
    /// Tag EXIF `Orientation` (1-8), usado para corregir la rotación de la miniatura.
    #[serde(skip)]
    pub orientation: Option<u32>,
    /// Bytes JPEG de la miniatura embebida en IFD1, si el archivo trae una.
    #[serde(skip)]
    pub embedded_thumbnail: Option<Vec<u8>>,
}

/// Lee los metadatos EXIF de `path`. Devuelve `ExifData` por defecto (todos los
/// campos en `None`) si el archivo no trae EXIF legible — no es un error fatal,
/// `init` sigue adelante sin esos datos.
pub fn read(path: &Path) -> ExifData {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ExifData::default(),
    };
    let mut reader = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return ExifData::default(),
    };

    let iso = exif
        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));

    let shutter_speed = exif
        .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string());

    let aperture = exif
        .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Rational(v) => v.first().map(|r| r.to_f64()),
            _ => None,
        });

    let focal_length = exif
        .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Rational(v) => v.first().map(|r| r.to_f64()),
            _ => None,
        });

    let lens = exif
        .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string());

    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));

    let embedded_thumbnail = extract_embedded_thumbnail(&exif);

    ExifData {
        iso,
        shutter_speed,
        aperture,
        focal_length,
        lens,
        orientation,
        embedded_thumbnail,
    }
}

/// Extrae la miniatura JPEG estándar de IFD1 (`JPEGInterchangeFormat`/`Length`).
///
/// Nota: esto cubre la miniatura EXIF estándar única. Cuando una cámara guarda
/// *varias* miniaturas embebidas en el MakerNote (común en Canon/Nikon), este
/// crate no las expone — `kamadak-exif` no parsea MakerNotes propietarios — así
/// que no se implementa la selección "de mayor resolución" entre varias
/// miniaturas que pide fase1-ingesta.md. Es una simplificación respecto al spec
/// que vale la pena revisar con el usuario si la calidad de miniatura resulta
/// insuficiente en la práctica.
fn extract_embedded_thumbnail(exif: &exif::Exif) -> Option<Vec<u8>> {
    let offset_field = exif.get_field(exif::Tag::JPEGInterchangeFormat, exif::In::THUMBNAIL)?;
    let length_field =
        exif.get_field(exif::Tag::JPEGInterchangeFormatLength, exif::In::THUMBNAIL)?;
    let offset = offset_field.value.get_uint(0)? as usize;
    let length = length_field.value.get_uint(0)? as usize;
    let buf = exif.buf();
    buf.get(offset..offset + length).map(|s| s.to_vec())
}
