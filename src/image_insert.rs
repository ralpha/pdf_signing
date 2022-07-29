use crate::{image_xobject::ImageXObject, rectangle::Rectangle, Error};
use lopdf::{
    content::{Content, Operation},
    Object, ObjectId,
};
use std::io::Read;

pub trait InsertImage {
    fn add_object<T: Into<Object>>(&mut self, object: T) -> ObjectId;

    /// Add image to pdf as XObject.
    /// The image will not be visible.
    /// Return the ObjectId of the image.
    fn add_image_as_form_xobject<R: Read>(
        &mut self,
        image_reader: R,
        image_name: &str,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        use lopdf::{Object::*, Stream};
        // Load image
        let image_decoder = png::Decoder::new(image_reader);
        let (mut image_xobject, mask_xobject) = ImageXObject::try_from(image_decoder)?;
        // Add object to object list
        if let Some(mask_xobject) = mask_xobject {
            let mask_xobject_id = self.add_object(mask_xobject);
            image_xobject.s_mask = Some(mask_xobject_id);
        }
        let image_xobject_id = self.add_object(image_xobject);

        let position = (0, 0);
        let size = (rect.x2 - rect.x1, rect.y2 - rect.y1);

        // Dictionary
        let form_xobject = lopdf::Dictionary::from_iter(vec![
            ("Type", Name("XObject".as_bytes().to_vec())),
            ("Subtype", Name("Form".as_bytes().to_vec())),
            // ("FormType", Integer(1)),
            (
                "Resources",
                Dictionary(lopdf::Dictionary::from_iter(vec![(
                    "XObject",
                    Dictionary(lopdf::Dictionary::from_iter(vec![(
                        image_name,
                        Reference(image_xobject_id),
                    )])),
                )])),
            ),
            (
                "BBox",
                Array(vec![0i32.into(), 0i32.into(), size.0.into(), size.1.into()]),
            ),
        ]);

        // Stream
        let mut content = Content {
            operations: Vec::<Operation>::new(),
        };
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
            vec![Name(image_name.as_bytes().to_vec())],
        ));
        // `Q` = Restore graphics state
        content.operations.push(Operation::new("Q", vec![]));

        let content_data = Content::encode(&content)?;

        // Return the form xobject
        Ok(self.add_object(Stream::new(form_xobject, content_data)))
    }
}
