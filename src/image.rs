use base64::prelude::*;
use std::io::Cursor;
use thiserror::Error;

pub fn _image_to_url(path: &str) -> Result<String, ImageError> {
    let image = image::open(path)?;

    let mut buf = Vec::<u8>::new();
    image.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::WebP)?;
    let img_base64 = BASE64_STANDARD.encode(&buf);
    Ok(format!("data:image/webp;base64,{img_base64}"))
}

#[derive(Error, Debug)]
pub enum ImageError {
    #[error("couldn't write image to intermediate buffer before encoding")]
    ImageError(#[from] image::ImageError),
}
