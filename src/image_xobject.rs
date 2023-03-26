// This code is inspired by https://github.com/fschutt/printpdf/blob/2bebdc65d06dafbe926ed4b43fedd10f966c59d3/src/xobject.rs

use crate::Error;
use lopdf::ObjectId;
use png::{BitDepth, ColorType};
use std::io::Read;

#[derive(Debug, Clone)]
pub struct ImageXObject {
    /// Width of the image (original width, not scaled width)
    pub width: u32,
    /// Height of the image (original height, not scaled height)
    pub height: u32,
    /// Color space (Greyscale, RGB, CMYK)
    pub color_space: ColorType,
    /// Bits per color component (1, 2, 4, 8, 16) - 1 for black/white, 8 Greyscale / RGB, etc.
    /// If using a JPXDecode filter (for JPEG images), this can be inferred from the image data
    pub bits_per_component: BitDepth,
    /// Should the image be interpolated when scaled?
    pub interpolate: bool,
    /// The actual data from the image
    pub image_data: Vec<u8>,
    /// Image used as a soft mask. (transparency)
    pub s_mask: Option<ObjectId>,
}

impl ImageXObject {
    // TODO: remove `unwrap`s
    /// Returns 1 or 2 images. The first is the color images.
    /// The second is (if present) the mask/alpha channel of the image.
    pub fn try_from<R: Read>(
        image_decoder: png::Decoder<R>,
    ) -> Result<(Self, Option<Self>), Error> {
        // Load image
        let mut image_reader = image_decoder.read_info().unwrap();
        // Allocate the output buffer.
        let mut buf = vec![0; image_reader.output_buffer_size()];
        // Read the next frame. An APNG might contain multiple frames.
        let info = image_reader.next_frame(&mut buf).unwrap();
        // Grab the bytes of the image.
        let image_data = Vec::from(&buf[..info.buffer_size()]);

        let mut color_type = info.color_type;
        let (image_color_data, alpha_data) = match info.color_type {
            ColorType::Rgba => {
                color_type = ColorType::Rgb;
                (
                    Self::rgba_to_rgb(&image_data),
                    Some(Self::rgba_to_a(&image_data)),
                )
            }
            ColorType::GrayscaleAlpha => {
                // TODO split alpha channel
                color_type = ColorType::Grayscale;
                (
                    Self::grayscale_alpha_to_grayscale(&image_data),
                    Some(Self::grayscale_alpha_to_grayscale(&image_data)),
                )
            }
            _ => (image_data, None),
        };

        Ok((
            Self {
                width: info.width,
                height: info.height,
                color_space: color_type,
                bits_per_component: info.bit_depth,
                image_data: image_color_data,
                interpolate: false,
                s_mask: None, // This should be filled in later
            },
            alpha_data.map(|alpha_data| Self {
                width: info.width,
                height: info.height,
                color_space: ColorType::Grayscale,
                bits_per_component: info.bit_depth,
                image_data: alpha_data,
                interpolate: false,
                s_mask: None,
            }),
        ))
    }

    // NOTE: This function only works for a bit depth of 8.
    fn rgba_to_rgb(data: &[u8]) -> Vec<u8> {
        let mut temp_counter = 0;
        let mut temp = [0u8; 3];
        // Doing `/4*3` will just make things to complicated and images will be small anyway.
        let mut output = Vec::with_capacity(data.len());
        for byte in data {
            match temp_counter {
                0..=2 => {
                    // Store value r, g or b.
                    temp[temp_counter] = *byte;
                    // Increase counter
                    temp_counter += 1;
                }
                _ => {
                    // Skip alpha
                    // and
                    // Color of 1 pixel is consumes (r, g, b, a)
                    output.extend_from_slice(&temp);
                    temp_counter = 0;
                }
            }
        }
        output
    }

    // NOTE: This function only works for a bit depth of 8.
    fn grayscale_alpha_to_grayscale(data: &[u8]) -> Vec<u8> {
        let mut temp_counter = 0;
        let mut temp = 0u8;
        let mut output = Vec::with_capacity(data.len() / 2);
        for byte in data {
            match temp_counter {
                0 => {
                    // Store value h.
                    temp = *byte;
                    // Increase counter
                    temp_counter += 1;
                }
                _ => {
                    // Skip alpha
                    // and
                    // Color of 1 pixel is consumes (g, a)
                    output.push(temp);
                    temp_counter = 0;
                }
            }
        }
        output
    }

    // NOTE: This function only works for a bit depth of 8.
    fn rgba_to_a(data: &[u8]) -> Vec<u8> {
        let mut temp_counter = 0;
        let mut output = Vec::with_capacity(data.len() / 4);
        for byte in data {
            match temp_counter {
                0..=2 => {
                    // Skip r, g, b
                    // Increase counter
                    temp_counter += 1;
                }
                _ => {
                    // Store alpha
                    // and
                    // Color of 1 pixel is consumes (r, g, b, a)
                    output.extend_from_slice(&[*byte]);
                    temp_counter = 0;
                }
            }
        }
        output
    }
}

// Inspired and derived from: https://github.com/fschutt/printpdf/blob/2bebdc65d06dafbe926ed4b43fedd10f966c59d3/src/xobject.rs#L245
impl From<ImageXObject> for lopdf::Stream {
    fn from(image: ImageXObject) -> Self {
        use lopdf::Object::*;

        let cs: &'static str = match image.color_space {
            ColorType::Rgb => "DeviceRGB",
            ColorType::Grayscale => "DeviceGray",
            ColorType::Indexed => "Indexed",
            ColorType::Rgba | ColorType::GrayscaleAlpha => "DeviceN",
        };
        let identity_matrix: Vec<f32> = vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let bbox: lopdf::Object = Array(identity_matrix.into_iter().map(Real).collect());

        let mut dict = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".as_bytes().to_vec())),
            ("Subtype", Name("Image".as_bytes().to_vec())),
            ("Width", Integer(image.width as i64)),
            ("Height", Integer(image.height as i64)),
            ("Interpolate", image.interpolate.into()),
            ("BitsPerComponent", Integer(image.bits_per_component as i64)),
            ("ColorSpace", Name(cs.as_bytes().to_vec())),
            ("BBox", bbox),
        ]);
        if let Some(s_mask) = image.s_mask {
            dict.set("SMask", Reference(s_mask));
        }

        lopdf::Stream::new(dict, image.image_data)
    }
}

impl From<ImageXObject> for lopdf::Object {
    fn from(image: ImageXObject) -> Self {
        lopdf::Object::Stream(image.into())
    }
}
