use crate::Error;
use lopdf::{Document, Object, ObjectId};

pub(super) trait PdfObjectDeref {
    fn deref<'a>(&'a self, doc: &'a Document) -> Result<&'a Object, Error>;

    fn get_object_id(&self) -> Option<ObjectId>;
}

impl PdfObjectDeref for Object {
    fn deref<'a>(&'a self, doc: &'a Document) -> Result<&'a Object, Error> {
        match *self {
            Object::Reference(oid) => doc
                .objects
                .get(&oid)
                .ok_or_else(|| Error::Other(format!("PDF Error: NoSuchReference({:#?})", oid))),
            _ => Ok(self),
        }
    }

    fn get_object_id(&self) -> Option<ObjectId> {
        match *self {
            Object::Reference(ref id) => Some(*id),
            _ => None,
        }
    }
}
