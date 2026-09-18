mod error;
mod sanitizer;

#[cfg(test)]
mod tests;

pub use error::SanitizeError;
pub use sanitizer::{sanitize, verify_pdf, SanitizeReport, Stage};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ImageQuality {
    Low,
    #[default]
    Medium,
    High,
}

impl ImageQuality {
    pub fn jpeg_quality(&self) -> u8 {
        match self {
            ImageQuality::Low => 45,
            ImageQuality::Medium => 70,
            ImageQuality::High => 85,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct SanitizationSettings {
    pub remove_metadata: bool,
    pub remove_scripts: bool,
    pub remove_embedded_files: bool,
    pub compress_images: bool,
    pub image_quality: ImageQuality,
    pub strip_external_links: bool,
    pub font_subsetting: bool,
    pub max_concurrent: u32,
    pub output_folder: String,
}

impl Default for SanitizationSettings {
    fn default() -> Self {
        SanitizationSettings {
            remove_metadata: true,
            remove_scripts: true,
            remove_embedded_files: true,
            compress_images: false,
            image_quality: ImageQuality::Medium,
            strip_external_links: false,
            font_subsetting: false,
            max_concurrent: 4,
            output_folder: String::new(),
        }
    }
}
