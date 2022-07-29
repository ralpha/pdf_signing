#[derive(Debug)]
pub enum Error {
    LoPdfError(lopdf::Error),
    TryFromIntError(std::num::TryFromIntError),
    Other(String),
}

impl From<lopdf::Error> for Error {
    fn from(err: lopdf::Error) -> Self {
        Self::LoPdfError(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Self::Other(err)
    }
}
impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self::Other(err.to_owned())
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(err: std::num::TryFromIntError) -> Self {
        Error::TryFromIntError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::LoPdfError(lopdf::Error::from(err))
    }
}
