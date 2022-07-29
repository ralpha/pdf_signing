use crate::PDFSigningDocument;
use crate::{error::Error, UserSignatureInfo};
use chrono::Utc;
use lopdf::ObjectId;

impl PDFSigningDocument {
    // Change the signature to add extra info about the signing application
    pub(crate) fn add_general_info_to_signature(
        &mut self,
        signature_obj_id: ObjectId,
        new_appearance_id: ObjectId,
        user_signature_info: &UserSignatureInfo,
        _signature_name: &str,
    ) -> Result<(), Error> {
        use lopdf::{Object::*, StringFormat};
        let _root_obj_id = self
            .raw_document
            .get_prev_documents()
            .trailer
            .get(b"Root")?
            .as_reference()?;

        // Update Annot to include image
        // Update `AP` in `Kids`

        // This code assumes that the objects in Kids are References.
        // Get list of `Kids` ObjectIDs
        let form_dict = self
            .raw_document
            .get_prev_documents()
            .get_object(signature_obj_id)?
            .as_dict()?;
        let kids = if form_dict.has(b"Kids") {
            Some(form_dict.get(b"Kids")?.as_array()?)
        } else {
            None
        };
        // Convert `Option<&Vec<Object>>` to `Option<Vec<ObjectId>>`
        let kids: Option<Vec<ObjectId>> = match kids {
            Some(list) => {
                let mut new_list = vec![];
                for obj in list {
                    new_list.push(obj.as_reference()?);
                }
                Some(new_list)
            }
            None => None,
        };

        // TODO if kids is `None` we have to create it.
        if kids.is_none() {
            log::error!("Unimplemented state: `Kids` entry is missing in Signature.");
        }
        let kids = kids.unwrap_or_default();

        // Loop over Kids that are `Annot`.
        let mut found_and_replace_appearance = false;
        for child_obj_id in kids {
            let child_dict = self
                .raw_document
                .get_prev_documents()
                .get_object(child_obj_id)?
                .as_dict()?;

            if child_dict.get(b"Type")?.as_name_str()? == "Annot" {
                // Copy child to new incremental update
                self.raw_document
                    .opt_clone_object_to_new_document(child_obj_id)?;

                let child_dict_mut = self
                    .raw_document
                    .new_document
                    .get_object_mut(child_obj_id)?
                    .as_dict_mut()?;

                child_dict_mut.set(
                    "AP",
                    lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(vec![(
                        "N",
                        lopdf::Object::Reference(new_appearance_id),
                    )])),
                );
                // TODO Set the `F` value to 132: For docs see page 385.
                // This will `Locked` and `Print`
                found_and_replace_appearance = true;
            }
        }

        if !found_and_replace_appearance {
            log::error!("None of the `Kids` are of type `Annot`.");
        }

        // Update `V` tag in `FT = Sig`
        self.raw_document
            .opt_clone_object_to_new_document(signature_obj_id)?;

        // The `sign_dict.extend(` is bugged see: lopdf issue #120
        // TODO: Implementing locking of file
        // sign_dict.set(
        //     "Lock",
        //     Dictionary(lopdf::Dictionary::from_iter(vec![
        //         ("Type", Name("SigFieldLock".as_bytes().to_vec())),
        //         ("Action", Name("All".as_bytes().to_vec())),
        //     ])),
        // );

        // Get system time in UTC
        let now = Utc::now();

        let v_dictionary = Dictionary(lopdf::Dictionary::from_iter(vec![
            ("Type", Name("Sig".as_bytes().to_vec())),
            ("Filter", Name("Adobe.PPKLite".as_bytes().to_vec())),
            ("SubFilter", Name("adbe.pkcs7.detached".as_bytes().to_vec())),
            // The order of `ByteRange` and `Contents` is important.
            // They should not be moved or switched in ordering.
            (
                "ByteRange", // Set default value. This will later be filled in.
                Array(vec![
                    Integer(0),
                    Integer(10000), // byte of `<`
                    Integer(20000), // Byte of char after `>`
                    Integer(10000), // until end of file
                ]),
            ),
            (
                "Contents", // Will be filled in later
                String(vec![0u8; 9000], StringFormat::Hexadecimal),
            ),
            (
                "M",
                String(
                    now.format("D:%Y%m%d%H%M%S+00'00'")
                        .to_string()
                        .as_bytes()
                        .to_vec(),
                    StringFormat::Literal,
                ),
            ),
            (
                "Name",
                String(
                    user_signature_info.user_name.as_bytes().to_vec(),
                    StringFormat::Literal,
                ),
            ),
            (
                "Prob_Build",
                Dictionary(lopdf::Dictionary::from_iter(vec![
                    (
                        "Filter",
                        Dictionary(lopdf::Dictionary::from_iter(vec![
                            ("Name", Name("Adobe.PPKLite".as_bytes().to_vec())),
                            (
                                "Date",
                                String(
                                    "Mar  2 2022 20:56:26".as_bytes().to_vec(),
                                    StringFormat::Literal,
                                ),
                            ), // TODO: Date of build, no Cargo Env variable for this
                            ("R", Integer(0x0000_0001_0101)), // TODO encode version number, in hex
                        ])),
                    ),
                    (
                        "PubSec",
                        Dictionary(lopdf::Dictionary::from_iter(vec![
                            (
                                "Date",
                                String(
                                    "Mar  2 2022 20:56:26".as_bytes().to_vec(),
                                    StringFormat::Literal,
                                ),
                            ), // TODO
                            ("R", Integer(0x0000_0001_0101)), // TODO encode version number, in hex
                            ("NonEFontNoWarn", Boolean(true)),
                        ])),
                    ),
                    (
                        "App",
                        Dictionary(lopdf::Dictionary::from_iter(vec![
                            ("Name", Name("Rust PDF Signing".as_bytes().to_vec())),
                            ("R", Integer(0x0000_0001_0001)), // TODO encode version number, in hex
                            (
                                "OS",
                                Array(vec![Name(std::env::consts::OS.as_bytes().to_vec())]),
                            ),
                            (
                                "REx",
                                String(
                                    env!("CARGO_PKG_VERSION").as_bytes().to_vec(),
                                    StringFormat::Literal,
                                ),
                            ), // Semversion number
                        ])),
                    ),
                ])),
            ),
            // (
            //     "Reference",
            //     Array(vec![Dictionary(lopdf::Dictionary::from_iter(vec![
            //         ("Type", Name("SigRef".as_bytes().to_vec())),
            //         ("TransformMethod", Name("FieldMDP".as_bytes().to_vec())), // TODO: maybe DocMDP?
            //         (
            //             "TransformParams",
            //             Dictionary(lopdf::Dictionary::from_iter(vec![
            //                 ("Type", Name("TransformParams".as_bytes().to_vec())),
            //                 ("Action", Name("Include".as_bytes().to_vec())),
            //                 (
            //                     "Fields",
            //                     Array(vec![String(
            //                         signature_name.as_bytes().to_vec(),
            //                         StringFormat::Literal,
            //                     )]),
            //                 ),
            //                 ("V", Name("1.2".as_bytes().to_vec())),
            //             ])),
            //         ),
            //         ("Data", Reference(root_obj_id)), // Signature is over whole document
            //         ("DigestMethod", Name("SHA1".as_bytes().to_vec())),
            //         (
            //             "DigestValue",
            //             String(vec![0u8; 20], StringFormat::Hexadecimal),
            //         ), // TODO: Sha1 = 20 bytes
            //         ("DigestLocation", Array(vec![Integer(1500), Integer(34)])), // TODO
            //     ]))]),
            // ),
        ]));

        // Add `V` as new object
        let v_ref = self.raw_document.new_document.add_object(v_dictionary);

        let sign_dict = self
            .raw_document
            .new_document
            .get_object_mut(signature_obj_id)?
            .as_dict_mut()?;

        sign_dict.set("V", Reference(v_ref));

        Ok(())
    }
}
