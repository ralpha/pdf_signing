mod acro_form;
mod byte_range;
mod digitally_sign;
mod error;
mod image_insert;
mod image_insert_to_page;
mod image_xobject;
mod lopdf_utils;
mod pdf_object;
mod rectangle;
mod signature_image;
mod signature_info;
mod user_signature_info;

use acro_form::AcroForm;
use byte_range::ByteRange;
use image_insert::InsertImage;
use image_insert_to_page::InsertImageToPage;
use lopdf::{
    content::{Content, Operation},
    Document, IncrementalDocument, Object, ObjectId,
};
use pdf_object::PdfObjectDeref;
use std::collections::HashMap;
use std::{fs::File, path::Path};

pub use error::Error;
pub use lopdf;
pub use user_signature_info::{UserFormSignatureInfo, UserSignatureInfo};

/// The whole PDF document. This struct only loads part of the document on demand.
#[derive(Debug, Clone)]
pub struct PDFSigningDocument {
    raw_document: IncrementalDocument,
    file_name: String,
    /// Link between the image name saved and the objectId of the image.
    /// This is used to reduce the amount of copies of the images in the pdf file.
    image_signature_object_id: HashMap<String, ObjectId>,

    acro_form: Option<Vec<AcroForm>>,
}

impl PDFSigningDocument {
    fn new(raw_document: IncrementalDocument, file_name: String) -> Self {
        PDFSigningDocument {
            raw_document,
            file_name,
            image_signature_object_id: HashMap::new(),
            acro_form: None,
        }
    }

    pub fn copy_from(&mut self, other: Self) {
        self.raw_document = other.raw_document;
        self.file_name = other.file_name;
        // Do not replace `image_signature_object_id`
        // We want to keep this so we can do optimization.
        self.acro_form = other.acro_form;
    }

    pub fn read_from<R: std::io::Read>(reader: R, file_name: String) -> Result<Self, Error> {
        let raw_doc = IncrementalDocument::load_from(reader)?;
        Ok(Self::new(raw_doc, file_name))
    }

    pub fn read<P: AsRef<Path>>(path: P, file_name: String) -> Result<Self, Error> {
        let raw_doc = IncrementalDocument::load(path)?;
        Ok(Self::new(raw_doc, file_name))
    }

    pub fn load_all(&mut self) -> Result<(), Error> {
        self.load_acro_form()
    }

    pub fn load_acro_form(&mut self) -> Result<(), Error> {
        if self.acro_form.is_none() {
            self.acro_form = Some(AcroForm::load_all_forms(
                self.raw_document.get_prev_documents(),
            )?);
        } else {
            log::info!("Already Loaded Acro Form.");
        }
        Ok(())
    }

    /// Save document to file
    pub fn save_document<P: AsRef<Path>>(&self, path: P) -> Result<File, Error> {
        // Create clone so we can compress the clone, not the original.
        let mut raw_document = self.raw_document.clone();
        raw_document.new_document.compress();
        Ok(raw_document.save(path)?)
    }

    /// Write document to Writer or buffer
    pub fn write_document<W: std::io::Write>(&self, target: &mut W) -> Result<(), Error> {
        // Create clone so we can compress the clone, not the original.
        let mut raw_document = self.raw_document.clone();
        raw_document.new_document.compress();
        raw_document.save_to(target)?;
        Ok(())
    }

    pub fn get_incr_document_ref(&self) -> &IncrementalDocument {
        &self.raw_document
    }

    pub fn get_prev_document_ref(&self) -> &Document {
        self.raw_document.get_prev_documents()
    }

    pub fn get_new_document_ref(&self) -> &Document {
        &self.raw_document.new_document
    }

    pub fn sign_document(
        &mut self,
        users_signature_info: Vec<UserSignatureInfo>,
    ) -> Result<Vec<u8>, Error> {
        self.load_all()?;
        // Set PDF version, version 1.5 is the minimum version required.
        self.raw_document.new_document.version = "1.5".to_owned();

        // loop over AcroForm elements
        let mut acro_forms = self.acro_form.clone();
        let mut last_binary_pdf = None;

        // Take the first form field (if there is any)
        let mut form_field_current = acro_forms.as_ref().and_then(|list| list.first().cloned());
        let mut form_field_index = 0;

        // Covert `Vec<UserSignatureInfo>` to `HashMap<String, UserSignatureInfo>`
        let users_signature_info_map: HashMap<String, UserSignatureInfo> = users_signature_info
            .iter()
            .map(|info| (info.user_id.clone(), info.clone()))
            .collect();

        // Make sure we never end up in an infinite loop, should not happen.
        // But better safe then sorry.
        let mut loop_counter: u16 = 0;
        // Loop over all the form fields and sign them one by one.
        while let Some(form_field) = form_field_current {
            loop_counter += 1;
            if loop_counter >= 10000 {
                log::error!(
                    "Infinite loop detected and prevented. Please check file: `{}`.",
                    self.file_name
                );
                break;
            }
            // Check if it is a signature and it is already signed.
            if !form_field.is_empty_signature() {
                // Go to next form field if pdf did not change
                form_field_index += 1;
                form_field_current = acro_forms
                    .as_ref()
                    .and_then(|list| list.get(form_field_index).cloned());
                // Go back to start of while loop
                continue;
            }

            // TODO: Debug code, can be removed
            // if form_field_index == 1 {
            //     form_field_index += 1;
            //     form_field_current = acro_forms
            //         .as_ref()
            //         .and_then(|list| list.get(form_field_index).cloned());
            //     continue;
            // }

            // Update pdf (when nothing else is incorrect)
            // Insert signature images into pdf itself.
            let pdf_document_user_info_opt =
                self.add_signature_images(form_field, &users_signature_info_map)?;

            // PDF has been updated, now we need to digitally sign it.
            if let Some((pdf_document_image, user_form_info)) = pdf_document_user_info_opt {
                // Digitally sign the document using a cert.
                let user_info = users_signature_info_map
                    .get(&user_form_info.user_id)
                    .ok_or_else(|| Error::Other("User was not found".to_owned()))?;

                let new_binary_pdf = pdf_document_image.digitally_sign_document(user_info)?;
                // Reload file
                self.copy_from(Self::read_from(
                    &*new_binary_pdf,
                    pdf_document_image.file_name,
                )?);
                self.load_all()?;
                self.raw_document.new_document.version = "1.5".to_owned();
                acro_forms = self.acro_form.clone();
                // Set as return value
                last_binary_pdf = Some(new_binary_pdf);
                // Reset form field index
                form_field_index = 0;
            } else {
                // Go to next form field because pdf did not change
                form_field_index += 1;
            }

            // Load next form field (or set to `0` depending on index.)
            form_field_current = acro_forms
                .as_ref()
                .and_then(|list| list.get(form_field_index).cloned());
        }

        match last_binary_pdf {
            Some(last_binary_pdf) => Ok(last_binary_pdf),
            None => {
                // No signing done, so just return initial document.
                Ok(self.raw_document.get_prev_documents_bytes().to_vec())
            }
        }
    }

    // pub fn add_signature_to_form<R: Read>(
    //     &mut self,
    //     image_reader: R,
    //     image_name: &str,
    //     page_id: ObjectId,
    //     form_id: ObjectId,
    // ) -> Result<ObjectId, Error> {
    //     let rect = Rectangle::get_rectangle_from_signature(form_id, &self.raw_document)?;
    //     let image_object_id_opt = self.image_signature_object_id.get(image_name).cloned();
    //     Ok(if let Some(image_object_id) = image_object_id_opt {
    //         // Image was already added so we can reuse it.
    //         self.add_image_to_page_only(image_object_id, image_name, page_id, rect)?
    //     } else {
    //         // Image was not added already so we need to add it in full
    //         let image_object_id = self.add_image(image_reader, image_name, page_id, rect)?;
    //         // Add signature to map
    //         self.image_signature_object_id
    //             .insert(image_name.to_owned(), image_object_id);
    //         image_object_id
    //     })
    // }
}

impl InsertImage for PDFSigningDocument {
    fn add_object<T: Into<Object>>(&mut self, object: T) -> ObjectId {
        self.raw_document.new_document.add_object(object)
    }
}

impl InsertImageToPage for PDFSigningDocument {
    fn add_xobject<N: Into<Vec<u8>>>(
        &mut self,
        page_id: ObjectId,
        xobject_name: N,
        xobject_id: ObjectId,
    ) -> Result<(), Error> {
        Ok(self
            .raw_document
            .add_xobject(page_id, xobject_name, xobject_id)?)
    }

    fn opt_clone_object_to_new_document(&mut self, object_id: ObjectId) -> Result<(), Error> {
        Ok(self
            .raw_document
            .opt_clone_object_to_new_document(object_id)?)
    }

    fn add_to_page_content(
        &mut self,
        page_id: ObjectId,
        content: Content<Vec<Operation>>,
    ) -> Result<(), Error> {
        Ok(self
            .raw_document
            .new_document
            .add_to_page_content(page_id, content)?)
    }
}
