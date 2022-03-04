mod error;
mod image_xobject;
mod pdf_object;

use image_xobject::ImageXObject;
use lopdf::{Document, ObjectId};
use pdf_object::PdfObjectDeref;
use std::{collections::HashMap, io::Read};

pub use error::Error;
pub use lopdf;

#[derive(Debug, Clone, Default)]
struct Rectangle {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// The whole PDF document. This struct only loads part of the document on demand.
#[derive(Debug, Clone)]
pub struct PDFSigningDocument {
    raw_document: Document,
    /// Link between the image name saved and the objectId of the image.
    /// This is used to reduce the amount of copies of the images in the pdf file.
    image_signature_object_id: HashMap<String, ObjectId>,
}

impl PDFSigningDocument {
    pub fn new(raw_document: Document) -> Self {
        PDFSigningDocument {
            raw_document,
            image_signature_object_id: HashMap::new(),
        }
    }

    pub fn finished(self) -> Document {
        self.raw_document
    }

    pub fn get_document_ref(&self) -> &Document {
        &self.raw_document
    }

    pub fn add_signature_to_form<R: Read>(
        &mut self,
        image_reader: R,
        image_name: &str,
        page_id: ObjectId,
        form_id: ObjectId,
    ) -> Result<ObjectId, Error> {
        let rect = Self::get_rectangle(form_id, &self.raw_document)?;
        let image_object_id_opt = self.image_signature_object_id.get(image_name).cloned();
        Ok(if let Some(image_object_id) = image_object_id_opt {
            // Image was already added so we can reuse it.
            self.add_image_to_page_only(image_object_id, image_name, page_id, rect)?
        } else {
            // Image was not added already so we need to add it in full
            let image_object_id = self.add_image(image_reader, image_name, page_id, rect)?;
            // Add signature to map
            self.image_signature_object_id
                .insert(image_name.to_owned(), image_object_id);
            image_object_id
        })
    }

    /// For an AcroForm find the rectangle on the page.
    fn get_rectangle(form_id: ObjectId, raw_doc: &Document) -> Result<Rectangle, Error> {
        let mut rect = None;
        // Get kids
        let form_dict = raw_doc.get_object(form_id)?.as_dict()?;
        let kids = if form_dict.has(b"Kids") {
            Some(form_dict.get(b"Kids")?.as_array()?)
        } else {
            None
        };

        if let Some(kids) = kids {
            for child in kids {
                let child_dict = child.deref(raw_doc)?.as_dict()?;
                // Child should be of `Type` `Annot` for Annotation.
                if child_dict.has(b"Rect") {
                    let child_rect = child_dict.get(b"Rect")?.as_array()?;
                    if child_rect.len() >= 4 {
                        // Found a reference, set as return value
                        rect = Some(Rectangle {
                            x1: child_rect[0].as_f64()?,
                            y1: child_rect[1].as_f64()?,
                            x2: child_rect[2].as_f64()?,
                            y2: child_rect[3].as_f64()?,
                        });
                    }
                }
            }
        }

        rect.ok_or_else(|| Error::Other("AcroForm: Rectangle not found.".to_owned()))
    }

    fn add_image<R: Read>(
        &mut self,
        image_reader: R,
        image_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        // Load image
        let image_decoder = png::Decoder::new(image_reader);
        let (mut image_xobject, mask_xobject) = ImageXObject::try_from(image_decoder)?;
        // Add object to object list
        if let Some(mask_xobject) = mask_xobject {
            let mask_xobject_id = self.raw_document.add_object(mask_xobject);
            image_xobject.s_mask = Some(mask_xobject_id);
        }
        let image_xobject_id = self.raw_document.add_object(image_xobject);
        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.raw_document
            .add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    fn add_image_to_page_only(
        &mut self,
        image_xobject_id: ObjectId,
        image_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.raw_document
            .add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    // The image must already be added to the object list!
    // Please use `add_image` instead.
    fn add_image_to_page_stream(
        &mut self,
        xobject_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<(), Error> {
        use lopdf::{content::Operation, Object::*};
        let mut content = self.raw_document.get_and_decode_page_content(page_id)?;
        let position = (rect.x1, rect.y1);
        let size = (rect.x2 - rect.x1, rect.y2 - rect.y1);
        // The following lines use commands: see p643 (Table A.1) for more info
        // `q` = Save graphics state
        content.operations.push(Operation::new("q", vec![]));
        // `cm` = Concatenate matrix to current transformation matrix
        content.operations.push(Operation::new(
            "cm",
            vec![
                size.0.into(),
                0i32.into(),
                0i32.into(),
                size.1.into(),
                position.0.into(),
                position.1.into(),
            ],
        ));
        // `Do` = Invoke named XObject
        content.operations.push(Operation::new(
            "Do",
            vec![Name(xobject_name.as_bytes().to_vec())],
        ));
        // `Q` = Restore graphics state
        content.operations.push(Operation::new("Q", vec![]));

        self.raw_document
            .change_page_content(page_id, content.encode()?)?;

        Ok(())
    }
}
