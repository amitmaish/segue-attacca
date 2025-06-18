use base64::prelude::*;
use image::imageops::FilterType;
use std::io::Cursor;
use thiserror::Error;

#[derive(Default, Debug, PartialEq, Eq)]
pub enum Image {
    Some(Box<str>),
    Loading,
    #[default]
    None,
}

pub fn image_to_url(path: &str, resize: Option<(u32, u32)>) -> Result<String, ImageError> {
    let mut image = image::open(path)?;
    if let Some((x, y)) = resize {
        image = image.resize(x, y, FilterType::CatmullRom);
    }

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
