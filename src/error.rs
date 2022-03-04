#[derive(Debug)]
pub enum Error {
    LoPdfError(lopdf::Error),
    Other(String),
}

impl From<lopdf::Error> for Error {
    fn from(err: lopdf::Error) -> Self {
        Self::LoPdfError(err)
    }
}
