use image::{DynamicImage, ImageError, ImageFormat};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Clone, Serialize, Deserialize)]
pub struct Texture {
    data: Vec<u8>,
}

impl Texture {
    pub fn new(image: DynamicImage) -> Self {
        let mut cursor = Cursor::new(Vec::new());
        image
            .to_rgba8()
            .write_to(&mut cursor, ImageFormat::Png)
            .expect("Failed to save image");
        Self {
            data: cursor.into_inner(),
        }
    }

    pub fn decode(&self) -> Result<DynamicImage, ImageError> {
        image::load_from_memory_with_format(&self.data, ImageFormat::Png)
    }
}
