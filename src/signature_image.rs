use crate::acro_form::AcroForm;
use crate::error::Error;
use crate::pdf_object::PdfObjectDeref;
use crate::rectangle::Rectangle;
use crate::user_signature_info::{UserFormSignatureInfo, UserSignatureInfo};
use crate::{InsertImage, PDFSigningDocument};
use lopdf::ObjectId;
use std::collections::HashMap;

impl PDFSigningDocument {
    pub(crate) fn add_signature_images(
        &mut self,
        signature_element: AcroForm,
        users_signature_info_map: &HashMap<String, UserSignatureInfo>,
    ) -> Result<Option<(Self, UserFormSignatureInfo)>, Error> {
        let mut pdf_signing_document = self.clone();

        // Check if it is a signature
        if !signature_element.is_empty_signature() {
            log::warn!("Can not create signing for completed signatures");
            return Ok(None);
        }

        let form_object_id = signature_element.get_object_id().ok_or_else(|| {
            Error::Other("AcroForm object is not a indirect reference.".to_owned())
        })?;

        let rect = pdf_signing_document.get_rectangle_from_form(form_object_id)?;
        let encoded_data = signature_element.get_partial_field_name();
        if encoded_data.is_none() {
            // Skip because this form field might not be created by us.
            log::warn!("Signature does not contain encoded data");
            return Ok(None);
        }
        let encoded_data = encoded_data.unwrap();
        // Decode data (from base64 to Vec<u8>)
        let decoded_data = match base64::decode(encoded_data) {
            Ok(decoded_data) => decoded_data,
            Err(err) => {
                log::warn!(
                    "Form alternate field name is not a base64 encoded field. Err: {}",
                    err
                );
                return Ok(None);
            }
        };
        // Decode to JSON
        let json_data: UserFormSignatureInfo = match serde_json::from_slice(&decoded_data) {
            Ok(json_data) => json_data,
            Err(err) => {
                log::warn!(
                    "Form alternate field name does not contain json data. Err: {}",
                    err
                );
                return Ok(None);
            }
        };

        // Get correct user signature info
        if let Some(user_signature_info) = users_signature_info_map.get(&json_data.user_id) {
            // Insert the signature into the PDF
            let image_name = format!("UserSignature{}", user_signature_info.user_id);
            let image_object_id = if let Some(image_object_id) = self
                .image_signature_object_id
                .get(&user_signature_info.user_id)
            {
                // Image was already added so we can reuse it.
                *image_object_id
            } else {
                // Image was not added already so we need to add it in full
                let image_object_id = pdf_signing_document.add_image_as_form_xobject(
                    &*user_signature_info.user_signature,
                    &image_name,
                    rect,
                )?;

                // Add signature to map
                self.image_signature_object_id
                    .insert(user_signature_info.user_id.clone(), image_object_id);
                image_object_id
            };
            log::info!(
                "Inserted signature for user `{}` into `{}` objId: `({},{})`.",
                user_signature_info.user_id,
                pdf_signing_document.file_name,
                image_object_id.0,
                image_object_id.1,
            );

            // Add info to signature object
            pdf_signing_document.add_general_info_to_signature(
                form_object_id,
                image_object_id,
                user_signature_info,
                encoded_data,
            )?;
        } else {
            log::error!(
                "User info required for user `{}` but was not provided.",
                json_data.user_id
            );
            return Ok(None);
        }

        Ok(Some((pdf_signing_document, json_data)))
    }

    /// For an AcroForm find the rectangle on the page.
    fn get_rectangle_from_form(&self, form_id: ObjectId) -> Result<Rectangle, Error> {
        let mut rect = None;
        // Get kids
        let form_dict = self
            .raw_document
            .get_prev_documents()
            .get_object(form_id)?
            .as_dict()?;
        let kids = if form_dict.has(b"Kids") {
            Some(form_dict.get(b"Kids")?.as_array()?)
        } else {
            None
        };

        if let Some(kids) = kids {
            for child in kids {
                let child_dict = child
                    .deref(self.raw_document.get_prev_documents())?
                    .as_dict()?;
                // Child should be of `Type` `Annot` for Annotation.
                if child_dict.has(b"Rect") {
                    let child_rect = child_dict.get(b"Rect")?.as_array()?;
                    if child_rect.len() >= 4 {
                        // Found a reference, set as return value
                        rect = Some(Rectangle {
                            x1: child_rect[0].as_f32()?,
                            y1: child_rect[1].as_f32()?,
                            x2: child_rect[2].as_f32()?,
                            y2: child_rect[3].as_f32()?,
                        });
                    }
                }
            }
        }

        rect.ok_or_else(|| Error::Other("AcroForm: Rectangle not found.".to_owned()))
    }
}
