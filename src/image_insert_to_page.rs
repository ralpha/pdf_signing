use crate::{image_xobject::ImageXObject, rectangle::Rectangle, Error, InsertImage};
use lopdf::{
    content::{Content, Operation},
    ObjectId,
};
use std::io::Read;

#[allow(dead_code)]
pub trait InsertImageToPage: InsertImage {
    fn add_xobject<N: Into<Vec<u8>>>(
        &mut self,
        page_id: ObjectId,
        xobject_name: N,
        xobject_id: ObjectId,
    ) -> Result<(), Error>;

    fn opt_clone_object_to_new_document(&mut self, object_id: ObjectId) -> Result<(), Error>;

    fn add_to_page_content(
        &mut self,
        page_id: ObjectId,
        content: Content<Vec<Operation>>,
    ) -> Result<(), Error>;

    /// Add image to a page.
    /// Return the ObjectId of the image.
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
            let mask_xobject_id = self.add_object(mask_xobject);
            image_xobject.s_mask = Some(mask_xobject_id);
        }
        let image_xobject_id = self.add_object(image_xobject);

        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    /// Add an already existing image to a page.
    /// Return the ObjectId of the image.
    fn add_image_to_page_only(
        &mut self,
        image_xobject_id: ObjectId,
        image_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    /// Add image to page stream.
    /// The image must already be added to the object list of the page!
    /// Please use `add_image` or `add_image_to_page_only` instead.
    fn add_image_to_page_stream(
        &mut self,
        xobject_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<(), Error> {
        use lopdf::Object::*;
        let mut content = Content {
            operations: Vec::<Operation>::new(),
        };
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

        self.opt_clone_object_to_new_document(page_id)?;
        self.add_to_page_content(page_id, content)?;

        Ok(())
    }
}
