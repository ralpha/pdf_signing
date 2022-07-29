//! `AcroForm` is the section of a pdf used to store info about forms.
//!
#![allow(unused_variables)] // TODO: remove, but requires implementing `InheritableFields`
#![allow(dead_code)] // TODO: remove, but requires implementing `InheritableFields

use crate::PdfObjectDeref;
use crate::{lopdf_utils, Error};
use lopdf::{Document, Object, ObjectId};

#[derive(Debug, Clone)]
pub(crate) struct AcroForm {
    object_id: Option<ObjectId>,
    partial_field_name: Option<String>,
    alternate_field_name: Option<String>,

    form_component: FormComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum FormComponent {
    /// Push buttons have no state
    Button,
    /// `selected` is the singular option from `options` that is selected
    Radio,
    /// The toggle state of the checkbox
    CheckBox,
    /// `selected` is the list of selected options from `options`
    ListBox,
    /// `selected` is the list of selected options from `options`
    ComboBox,
    /// User Text Input
    Text,
    /// Signature field, not signed.
    EmptySignature,
    /// Signature field, already signed.
    SignedSignature {
        r#type: Option<String>,
        filter: String,
        sub_filter: Option<String>,
        contents: Vec<u8>,
        cert: Option<Vec<Vec<u8>>>,
        byte_range: Vec<(u64, u64)>,
        // `Reference` not implemented.
        // `Changes` not implemented.
        /// The name of the person or authority signing the document.
        /// This value should be used only when it is not possible to extract the
        /// name from the signature.
        name: Option<String>,
        // `M` (date of signing) not implemented.
        // `Location` not implemented.
        // `Reason` not implemented.
        // `ContactInfo` not implemented.
        // `R` not implemented. (deprecated)
        // `V` not implemented.
        // `` not implemented.
        prod_build: Option<SignBuildDictionary>,
        prod_auth_time: Option<u64>,
        prod_auth_type: Option<String>,
    },
    /// Unknown fields have no state
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignBuildDictionary {
    pub name: String,
    pub date: String,
    pub r: u32,
    pub pre_release: bool,
    pub os: Vec<String>,
    pub non_e_font_no_warn: bool,
    pub trusted_mode: bool,
    // `V` not implemented. Deprecated in PDF 1.7
    pub r_ex: String,
}

#[derive(Debug, Clone, Default)]
struct InheritableFields {
    /// Technically an enum.
    /// Allowed values:
    /// - `Btn` (Button)
    /// - `Tx` (Text)
    /// - `Ch` (Choice)
    /// - `Sig` (Signature) (PDF 1.3)
    ft: Option<String>,
}

impl AcroForm {
    pub(crate) fn load_all_forms(raw_doc: &Document) -> Result<Vec<Self>, Error> {
        // Help with pdf structure:
        // https://pdfux.com/inspect-pdf/
        // Structure of pdf:
        // - Root (dictionary)
        //   - AcroForm (dictionary)
        //     - Fields (array)
        //       - <references>
        //       - ...
        //   - Names
        //   - Pages
        //   - ...

        // Get `Root` node
        let root = raw_doc.trailer.get(b"Root")?.deref(raw_doc)?.as_dict()?;

        // Check if document has forms.
        if !root.has(b"AcroForm") {
            log::info!("Document does not contain any forms.");
            return Ok(vec![]);
        }
        // Get `AcroForm` node
        let acro_form_dict = root.get(b"AcroForm")?.deref(raw_doc)?.as_dict()?;
        // Get `Fields` node
        let fields_list = acro_form_dict.get(b"Fields")?.deref(raw_doc)?.as_array()?;

        // Fields can be a hierarchy, so need to be parsed this way.
        let empty_inherit_root = InheritableFields::default();
        Self::load_field_list(raw_doc, fields_list, empty_inherit_root)
    }

    /// For an AcroForm find a reference to the page it is on.
    pub(crate) fn get_page_ref(&self, raw_doc: &Document) -> Result<ObjectId, Error> {
        let mut object_id = None;
        let kids = self.get_kids(raw_doc)?;

        if let Some(kids) = kids {
            for child in kids {
                let child_dict = child.deref(raw_doc)?.as_dict()?;
                // Child should be of `Type` `Annot` for Annotation.
                if child_dict.has(b"P") {
                    let child_page_ir = child_dict.get(b"P")?.get_object_id();
                    if let Some(page_ir) = child_page_ir {
                        // Found a reference, set as return value
                        object_id = Some(page_ir);
                    }
                }
            }
        }

        object_id.ok_or_else(|| Error::from("AcroForm: Page reference not found."))
    }

    fn get_kids<'a>(&self, raw_doc: &'a Document) -> Result<Option<&'a Vec<Object>>, Error> {
        let self_object_id = self
            .object_id
            .ok_or_else(|| Error::from("AcroForm object is not a indirect reference."))?;

        let form_dict = raw_doc.get_object(self_object_id)?.as_dict()?;
        if form_dict.has(b"Kids") {
            Ok(Some(form_dict.get(b"Kids")?.as_array()?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn get_partial_field_name(&self) -> Option<&str> {
        self.partial_field_name.as_deref()
    }

    pub(crate) fn get_alternate_field_name(&self) -> Option<&str> {
        self.alternate_field_name.as_deref()
    }

    pub(crate) fn get_form_component(&self) -> &FormComponent {
        &self.form_component
    }

    pub(crate) fn is_empty_signature(&self) -> bool {
        self.form_component == FormComponent::EmptySignature
    }

    pub(crate) fn get_object_id(&self) -> Option<ObjectId> {
        self.object_id
    }

    /// Parse a list if referenced in the hierarchy of `Root->AcroForm->Fields`.
    ///
    /// There are properties that can be inherited from the parents.
    fn load_field_list(
        raw_doc: &Document,
        list: &[Object],
        inherit: InheritableFields,
    ) -> Result<Vec<Self>, Error> {
        // Create list for results
        let mut form_fields = vec![];

        for field in list {
            let field_object_id = field.get_object_id();
            let field_dict = field.deref(raw_doc)?.as_dict()?;

            // Check if it has the `FT` field.
            if field_dict.has(b"FT") {
                let component = match field_dict.get(b"FT")?.as_name()? {
                    b"Btn" => {
                        // Not implemented, ignored
                        FormComponent::Button
                    }
                    b"Tx" => {
                        // Not implemented, ignored
                        FormComponent::Text
                    }
                    b"Ch" => {
                        // Not implemented, ignored
                        FormComponent::ComboBox
                    }
                    b"Sig" => {
                        // We do not check or store all info according to the spec.
                        // We do not check required fields or locks.

                        // Check if `SV` (seed value) is set
                        if field_dict.has(b"SV") {
                            log::warn!("`SV` is not supported for signatures.");
                        }

                        // Check if already signed
                        if field_dict.has(b"V") {
                            let sign_value_dict =
                                field_dict.get(b"V")?.deref(raw_doc)?.as_dict()?;
                            if sign_value_dict.has(b"Filter") || sign_value_dict.has(b"Contents") {
                                // Signature is already signed.
                                FormComponent::SignedSignature {
                                    r#type: lopdf_utils::as_option_name(
                                        sign_value_dict.get(b"Type").ok(),
                                    )?,
                                    filter: lopdf_utils::as_name(
                                        sign_value_dict.get(b"Filter").ok(),
                                    )?,
                                    sub_filter: lopdf_utils::as_option_name(
                                        sign_value_dict.get(b"SubFilter").ok(),
                                    )?,
                                    contents: lopdf_utils::as_byte_string(
                                        sign_value_dict.get(b"Contents").ok(),
                                    )?,
                                    cert: lopdf_utils::as_array_or_byte_string(
                                        sign_value_dict.get(b"Cert").ok(),
                                    )
                                    .ok(),
                                    byte_range: lopdf_utils::as_byte_range(
                                        sign_value_dict.get(b"ByteRange").ok(),
                                    )?,
                                    name: lopdf_utils::as_option_text_string(
                                        sign_value_dict.get(b"Name").ok(),
                                    )?,
                                    prod_build: None, // TODO
                                    prod_auth_time: lopdf_utils::as_option_integer(
                                        sign_value_dict.get(b"Prop_AuthTime").ok(),
                                    )?
                                    .map(u64::try_from)
                                    .transpose()?,
                                    prod_auth_type: lopdf_utils::as_option_name(
                                        sign_value_dict.get(b"Prop_AuthType").ok(),
                                    )?,
                                }
                            } else {
                                FormComponent::EmptySignature
                            }
                        } else {
                            FormComponent::EmptySignature
                        }
                    }
                    unknown_type => {
                        log::warn!(
                            "Found an unknown `FT`: {}",
                            String::from_utf8_lossy(unknown_type)
                        );
                        FormComponent::Unknown
                    }
                };
                form_fields.push(AcroForm {
                    object_id: field_object_id,
                    partial_field_name: lopdf_utils::as_option_text_string(
                        field_dict.get(b"T").ok(),
                    )?,
                    alternate_field_name: lopdf_utils::as_option_text_string(
                        field_dict.get(b"TU").ok(),
                    )?,
                    form_component: component,
                });
            }
        }
        Ok(form_fields)
    }
}
