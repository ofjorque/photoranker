//! Extracción de la miniatura normalizada (rotada + reescalada) usada como
//! fuente para pHash y métricas de calidad — ver docs/fase1-ingesta.md,
//! secciones 1 y 3.

use crate::exif::ExifData;
use image::{DynamicImage, ImageBuffer, Rgb};
use std::path::Path;

/// La extracción de miniatura falló (sin preview JPEG embebido reconocible ni
/// decode reducido de RAW soportado por `rawloader`) — la imagen queda con
/// `thumbnail_status='failed'`, ver fase1-ingesta.md sección 3.
#[derive(Debug)]
pub struct ThumbnailExtractionFailed;

/// Extrae y normaliza (corrige rotación EXIF, reescala a `preview_size` en el
/// lado mayor) la miniatura de `path`. Intenta en orden: (1) decodificar el
/// archivo directamente como imagen estándar (JPEG/PNG/etc.), (2) usar la
/// miniatura JPEG embebida en EXIF/IFD1, (3) decode reducido de RAW vía
/// `rawloader` con debayer simple 2x2 (sin demosaico avanzado — suficiente
/// para pHash/métricas, no para revelado real), (4) escanear el archivo en
/// busca de un JPEG completo embebido "a mano" (ver `scan_embedded_jpegs`) —
/// cubre formatos que `rawloader` no reconoce en absoluto (ej. Canon CR3,
/// contenedor ISO-BMFF; `rawloader` 0.37 no tiene ningún decoder para CR3,
/// ni parcial ni por modelo de cámara — falla el 100% de las veces, no
/// intermitentemente, ver issue upstream pedrocr/rawloader#23) pero que de
/// todos modos suelen embeber uno o más JPEGs completos (miniatura + preview
/// de mayor resolución) como blobs contiguos dentro del archivo.
pub fn extract_normalized(
    path: &Path,
    exif: &ExifData,
    preview_size: u32,
) -> Result<DynamicImage, ThumbnailExtractionFailed> {
    if let Ok(img) = image::open(path) {
        return Ok(normalize(img, exif.orientation, preview_size));
    }

    if let Some(bytes) = &exif.embedded_thumbnail
        && let Ok(img) = image::load_from_memory(bytes)
    {
        return Ok(normalize(img, exif.orientation, preview_size));
    }

    if let Ok(raw) = rawloader::decode_file(path)
        && let Some(img) = debayer(&raw)
    {
        let dynamic = DynamicImage::ImageRgb8(img);
        let rotated = apply_rawloader_orientation(dynamic, raw.orientation);
        return Ok(resize_max_side(rotated, preview_size));
    }

    if let Ok(bytes) = std::fs::read(path)
        && let Some(img) = scan_embedded_jpegs(&bytes)
    {
        return Ok(normalize(img, exif.orientation, preview_size));
    }

    Err(ThumbnailExtractionFailed)
}

/// Último recurso, deliberadamente genérico y heurístico: escanea `bytes`
/// buscando pares de marcadores JPEG SOI (`FF D8`) / EOI (`FF D9`) y
/// devuelve, entre todos los que decodifican con éxito vía el crate `image`,
/// el más grande en bytes (suele ser el preview de mayor resolución, no la
/// miniatura chica — los contenedores RAW que embeben más de un JPEG casi
/// siempre incluyen ambos). No entiende la estructura del contenedor (ISO-BMFF,
/// TIFF, ni cajas específicas de cada fabricante) — solo busca bytes que
/// *parecen* un JPEG completo y confía en que `image::load_from_memory`
/// rechace los falsos positivos (un SOI/EOI que no encierra un JPEG válido
/// simplemente no decodifica). Se prueba como último recurso porque es más
/// lento y menos preciso que un parser real del contenedor.
fn scan_embedded_jpegs(bytes: &[u8]) -> Option<DynamicImage> {
    let mut best: Option<(usize, DynamicImage)> = None;
    let mut pos = 0;
    while pos + 1 < bytes.len() {
        if bytes[pos] == 0xFF
            && bytes[pos + 1] == 0xD8
            && let Some(end) = find_eoi(bytes, pos + 2)
        {
            let candidate = &bytes[pos..end];
            if let Ok(img) =
                image::load_from_memory_with_format(candidate, image::ImageFormat::Jpeg)
                && best.as_ref().is_none_or(|(len, _)| candidate.len() > *len)
            {
                best = Some((candidate.len(), img));
            }
            pos = end;
            continue;
        }
        pos += 1;
    }
    best.map(|(_, img)| img)
}

/// Busca el próximo marcador EOI (`FF D9`) desde `from`. Devuelve el índice
/// justo después de él (para que el slice `[soi..eoi]` incluya ambos
/// marcadores, como espera un decoder JPEG).
fn find_eoi(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 1 < bytes.len() {
        if bytes[i] == 0xFF && bytes[i + 1] == 0xD9 {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}

fn normalize(img: DynamicImage, exif_orientation: Option<u32>, preview_size: u32) -> DynamicImage {
    let rotated = apply_exif_orientation(img, exif_orientation.unwrap_or(1));
    resize_max_side(rotated, preview_size)
}

fn resize_max_side(img: DynamicImage, preview_size: u32) -> DynamicImage {
    if img.width() <= preview_size && img.height() <= preview_size {
        return img;
    }
    img.resize(
        preview_size,
        preview_size,
        image::imageops::FilterType::Lanczos3,
    )
}

/// Aplica la corrección de rotación/espejado según el tag EXIF `Orientation` (1-8).
fn apply_exif_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn apply_rawloader_orientation(
    img: DynamicImage,
    orientation: rawloader::Orientation,
) -> DynamicImage {
    use rawloader::Orientation as O;
    match orientation {
        O::Normal | O::Unknown => img,
        O::HorizontalFlip => img.fliph(),
        O::Rotate180 => img.rotate180(),
        O::VerticalFlip => img.flipv(),
        O::Transpose => img.rotate90().fliph(),
        O::Rotate90 => img.rotate90(),
        O::Transverse => img.rotate270().fliph(),
        O::Rotate270 => img.rotate270(),
    }
}

/// Debayer simple por bloques 2x2 (promedio, sin interpolación) sobre datos
/// Bayer de `rawloader`. Suficiente para pHash/métricas de calidad, no para
/// un revelado fotográfico real. Devuelve `None` si el CFA no es un patrón
/// Bayer 2x2 estándar (ej. X-Trans 6x6) o si los datos no son enteros de 16 bits.
fn debayer(raw: &rawloader::RawImage) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    if raw.cpp != 1 || raw.cfa.width != 2 || raw.cfa.height != 2 {
        return None;
    }
    let rawloader::RawImageData::Integer(data) = &raw.data else {
        return None;
    };

    let (width, height) = (raw.width, raw.height);
    let out_width = (width / 2) as u32;
    let out_height = (height / 2) as u32;
    if out_width == 0 || out_height == 0 {
        return None;
    }

    let white = raw
        .whitelevels
        .iter()
        .copied()
        .max()
        .unwrap_or(u16::MAX)
        .max(1) as f64;
    let mut buf = ImageBuffer::new(out_width, out_height);

    for oy in 0..out_height {
        for ox in 0..out_width {
            let (mut r_sum, mut g_sum, mut b_sum) = (0f64, 0f64, 0f64);
            let (mut g_count, mut r_count, mut b_count) = (0u32, 0u32, 0u32);
            for dy in 0..2usize {
                for dx in 0..2usize {
                    let row = (oy as usize) * 2 + dy;
                    let col = (ox as usize) * 2 + dx;
                    let value = data[row * width + col] as f64;
                    match raw.cfa.color_at(row, col) {
                        0 => {
                            r_sum += value;
                            r_count += 1;
                        }
                        2 => {
                            b_sum += value;
                            b_count += 1;
                        }
                        _ => {
                            g_sum += value;
                            g_count += 1;
                        }
                    }
                }
            }
            let to_u8 = |sum: f64, count: u32| -> u8 {
                if count == 0 {
                    return 0;
                }
                let normalized = (sum / count as f64 / white).clamp(0.0, 1.0);
                // Aproximación de gamma sRGB (no revelado real, ver docs).
                (normalized.powf(1.0 / 2.2) * 255.0).round() as u8
            };
            buf.put_pixel(
                ox,
                oy,
                Rgb([
                    to_u8(r_sum, r_count),
                    to_u8(g_sum, g_count),
                    to_u8(b_sum, b_count),
                ]),
            );
        }
    }

    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;
    use std::io::Cursor;

    fn encode_jpeg(width: u32, height: u32, value: u8) -> Vec<u8> {
        let img = RgbImage::from_pixel(width, height, Rgb([value, value, value]));
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    /// Simula un archivo de contenedor que `image`/`rawloader` no saben
    /// interpretar (como un CR3 real) pero que igual trae un JPEG completo
    /// embebido en algún punto del archivo, rodeado de bytes arbitrarios.
    #[test]
    fn finds_jpeg_embedded_in_unparseable_container() {
        let jpeg = encode_jpeg(32, 24, 128);
        let mut container = vec![0u8; 50]; // cabecera arbitraria, no es un JPEG
        container.extend_from_slice(&jpeg);
        container.extend_from_slice(&[0xAA; 30]); // cola arbitraria

        let found = scan_embedded_jpegs(&container).expect("debe encontrar el JPEG embebido");
        assert_eq!(found.width(), 32);
        assert_eq!(found.height(), 24);
    }

    #[test]
    fn picks_the_largest_valid_jpeg_when_multiple_are_embedded() {
        let small = encode_jpeg(16, 16, 50); // miniatura chica
        let large = encode_jpeg(64, 48, 200); // preview de mayor resolución

        let mut container = Vec::new();
        container.extend_from_slice(&[0u8; 20]);
        container.extend_from_slice(&small);
        container.extend_from_slice(&[0u8; 20]);
        container.extend_from_slice(&large);
        container.extend_from_slice(&[0u8; 20]);

        let found = scan_embedded_jpegs(&container).expect("debe encontrar al menos un JPEG");
        assert_eq!(
            (found.width(), found.height()),
            (64, 48),
            "debe preferir el JPEG más grande en bytes, no el primero encontrado"
        );
    }

    #[test]
    fn returns_none_when_no_valid_jpeg_is_embedded() {
        // Bytes con un SOI/EOI "de casualidad" pero sin un JPEG válido en medio.
        let container = vec![0xFF, 0xD8, 1, 2, 3, 4, 5, 0xFF, 0xD9];
        assert!(scan_embedded_jpegs(&container).is_none());
    }

    #[test]
    fn returns_none_on_empty_or_tiny_input() {
        assert!(scan_embedded_jpegs(&[]).is_none());
        assert!(scan_embedded_jpegs(&[0xFF]).is_none());
    }
}
