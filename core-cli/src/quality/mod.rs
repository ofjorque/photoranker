//! Métricas objetivas de calidad, calculadas sobre la miniatura ya normalizada
//! (rotada + reescalada) en `init` — ver docs/fase1-ingesta.md, sección 2.

use image::{DynamicImage, GenericImageView, Rgba};
use imageproc::filter::laplacian_filter;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orientation {
    Portrait,
    Landscape,
    Square,
}

impl Orientation {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
            Orientation::Square => "square",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct QualityMetrics {
    pub sharpness: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub overexposed_pct: f64,
    pub underexposed_pct: f64,
    pub saturation: f64,
    pub colorfulness: f64,
    pub entropy: f64,
    pub average_r: u8,
    pub average_g: u8,
    pub average_b: u8,
    pub orientation: Orientation,
}

/// Calcula todas las métricas objetivas sobre la miniatura normalizada `img`.
/// Determinista, sin modelos de ML — ver la tabla de fórmulas en fase1-ingesta.md.
pub fn compute(img: &DynamicImage) -> QualityMetrics {
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    let gray = img.to_luma8();

    let pixel_count = (width as u64 * height as u64).max(1) as f64;

    let mut luminance_sum: f64 = 0.0;
    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut over_count: u64 = 0;
    let mut under_count: u64 = 0;
    let mut saturation_sum: f64 = 0.0;
    let mut histogram = [0u64; 256];
    let mut rg_values: Vec<f64> = Vec::with_capacity((width * height) as usize);
    let mut yb_values: Vec<f64> = Vec::with_capacity((width * height) as usize);

    for Rgba([r, g, b, _]) in rgba.pixels() {
        let (r, g, b) = (*r, *g, *b);
        let luminance = gray_from_rgb(r, g, b);
        luminance_sum += luminance as f64;
        histogram[luminance as usize] += 1;
        if luminance > 250 {
            over_count += 1;
        }
        if luminance < 5 {
            under_count += 1;
        }
        r_sum += r as u64;
        g_sum += g as u64;
        b_sum += b as u64;
        saturation_sum += hsv_saturation(r, g, b);

        let rf = r as f64;
        let gf = g as f64;
        let bf = b as f64;
        rg_values.push(rf - gf);
        yb_values.push(0.5 * (rf + gf) - bf);
    }

    let brightness = luminance_sum / pixel_count;
    let contrast = {
        let variance: f64 = histogram
            .iter()
            .enumerate()
            .map(|(v, &count)| count as f64 * (v as f64 - brightness).powi(2))
            .sum::<f64>()
            / pixel_count;
        variance.sqrt()
    };
    let overexposed_pct = 100.0 * over_count as f64 / pixel_count;
    let underexposed_pct = 100.0 * under_count as f64 / pixel_count;
    let saturation = saturation_sum / pixel_count;
    let average_r = (r_sum as f64 / pixel_count).round() as u8;
    let average_g = (g_sum as f64 / pixel_count).round() as u8;
    let average_b = (b_sum as f64 / pixel_count).round() as u8;

    let colorfulness = hasler_susstrunk_colorfulness(&rg_values, &yb_values);
    let entropy = shannon_entropy(&histogram, pixel_count);
    let sharpness = laplacian_variance(&gray);
    let orientation = orientation_from_dimensions(width, height);

    QualityMetrics {
        sharpness,
        brightness,
        contrast,
        overexposed_pct,
        underexposed_pct,
        saturation,
        colorfulness,
        entropy,
        average_r,
        average_g,
        average_b,
        orientation,
    }
}

/// Luminancia sRGB de 8 bits sin conversión a espacio lineal (aproximación
/// intencionalmente simple, ver fase1-ingesta.md).
fn gray_from_rgb(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64).round() as u8
}

fn hsv_saturation(r: u8, g: u8, b: u8) -> f64 {
    let (r, g, b) = (r as f64, g as f64, b as f64);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max <= 0.0 { 0.0 } else { (max - min) / max }
}

fn hasler_susstrunk_colorfulness(rg: &[f64], yb: &[f64]) -> f64 {
    let n = rg.len().max(1) as f64;
    let mean_rg = rg.iter().sum::<f64>() / n;
    let mean_yb = yb.iter().sum::<f64>() / n;
    let var_rg = rg.iter().map(|v| (v - mean_rg).powi(2)).sum::<f64>() / n;
    let var_yb = yb.iter().map(|v| (v - mean_yb).powi(2)).sum::<f64>() / n;
    (var_rg + var_yb).sqrt() + 0.3 * (mean_rg.powi(2) + mean_yb.powi(2)).sqrt()
}

fn shannon_entropy(histogram: &[u64; 256], pixel_count: f64) -> f64 {
    histogram
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / pixel_count;
            -p * p.log2()
        })
        .sum()
}

fn laplacian_variance(gray: &image::GrayImage) -> f64 {
    let filtered = laplacian_filter(gray);
    let n = filtered.pixels().len().max(1) as f64;
    let mean = filtered.pixels().map(|p| p.0[0] as f64).sum::<f64>() / n;
    filtered
        .pixels()
        .map(|p| (p.0[0] as f64 - mean).powi(2))
        .sum::<f64>()
        / n
}

fn orientation_from_dimensions(width: u32, height: u32) -> Orientation {
    if width == height {
        Orientation::Square
    } else if width > height {
        Orientation::Landscape
    } else {
        Orientation::Portrait
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn orientation_matches_dimensions() {
        assert_eq!(orientation_from_dimensions(100, 50), Orientation::Landscape);
        assert_eq!(orientation_from_dimensions(50, 100), Orientation::Portrait);
        assert_eq!(orientation_from_dimensions(80, 80), Orientation::Square);
    }

    #[test]
    fn uniform_image_has_zero_contrast_and_sharpness() {
        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 16, Rgba([128, 128, 128, 255])));
        let metrics = compute(&img);
        assert!(metrics.contrast < 1e-6);
        assert!(metrics.sharpness < 1e-6);
        assert_eq!(metrics.orientation, Orientation::Square);
    }

    #[test]
    fn pure_white_image_is_fully_overexposed() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255])));
        let metrics = compute(&img);
        assert!((metrics.overexposed_pct - 100.0).abs() < 1e-6);
        assert!((metrics.underexposed_pct).abs() < 1e-6);
    }
}
