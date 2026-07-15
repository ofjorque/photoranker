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
/// para pHash/métricas, no para revelado real).
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

    Err(ThumbnailExtractionFailed)
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
