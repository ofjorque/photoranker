//! pHash clásico (DCT, 64 bits) sobre la miniatura normalizada — ver
//! docs/fase1-ingesta.md, sección 1.

use image::DynamicImage;
use img_hash::{HashAlg, HasherConfig};

/// Longitud del hash en bits (32x32 → DCT → 8x8 coeficientes de baja frecuencia).
pub const HASH_BITS: u32 = 64;

/// Calcula el pHash de 64 bits y lo devuelve como string hexadecimal (para
/// guardar en `images.hash`).
///
/// `img_hash` depende de su propia versión (vieja) del crate `image`, distinta
/// de la que usa el resto del proyecto — se puentea vía bytes RGBA crudos en
/// vez de compartir el tipo `DynamicImage` entre ambas versiones.
pub fn compute(img: &DynamicImage) -> String {
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    let bridged = img_hash::image::RgbaImage::from_raw(width, height, rgba.into_raw())
        .expect("el buffer RGBA tiene el tamaño exacto width*height*4");

    let hasher = HasherConfig::new()
        .hash_size(8, 8)
        .hash_alg(HashAlg::Mean)
        .preproc_dct()
        .to_hasher();
    let hash = hasher.hash_image(&bridged);
    encode_hex(hash.as_bytes())
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Distancia de Hamming normalizada entre dos hashes hex (0.0 idénticos, 1.0
/// máxima distancia sobre `HASH_BITS` bits).
pub fn normalized_distance(hash_a: &str, hash_b: &str) -> Option<f64> {
    let a = decode_hex(hash_a)?;
    let b = decode_hex(hash_b)?;
    if a.len() != b.len() {
        return None;
    }
    let bits_diff: u32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    Some(bits_diff as f64 / HASH_BITS as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_hashes_have_zero_distance() {
        assert_eq!(
            normalized_distance("00ff00ff00ff00ff", "00ff00ff00ff00ff"),
            Some(0.0)
        );
    }

    #[test]
    fn fully_opposite_hashes_have_max_distance() {
        assert_eq!(
            normalized_distance("0000000000000000", "ffffffffffffffff"),
            Some(1.0)
        );
    }

    #[test]
    fn similar_quadrant_images_are_close_and_distinct_ones_are_far() {
        use image::{Rgb, RgbImage};

        // Bloques grandes (cuadrantes) para que la estructura sobreviva al
        // downsample de 64x64 -> 16x16 que hace el preprocesamiento DCT
        // (ruido por-píxel se promedia y desaparece; el pHash está diseñado
        // para ignorarlo, así que un patrón de alta frecuencia por-píxel no
        // sirve como fixture de prueba).
        fn quadrant_image(bright_quadrant: u8) -> RgbImage {
            let mut img = RgbImage::new(64, 64);
            for (x, y, p) in img.enumerate_pixels_mut() {
                let q = (if x < 32 { 0 } else { 1 }) + (if y < 32 { 0 } else { 2 });
                let v = if q == bright_quadrant { 240 } else { 20 };
                *p = Rgb([v, v, v]);
            }
            img
        }

        let a = quadrant_image(0);
        let mut b = a.clone();
        for (i, p) in b.pixels_mut().enumerate() {
            if i % 37 == 0 {
                p.0[0] = p.0[0].saturating_add(10);
            }
        }
        let g = quadrant_image(3);

        let ha = compute(&DynamicImage::ImageRgb8(a));
        let hb = compute(&DynamicImage::ImageRgb8(b));
        let hg = compute(&DynamicImage::ImageRgb8(g));

        assert_eq!(normalized_distance(&ha, &hb), Some(0.0));
        assert!(normalized_distance(&ha, &hg).unwrap() > 0.10);
    }
}
