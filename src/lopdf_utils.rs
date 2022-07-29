use crate::Error;
use lopdf::Object;

pub(crate) fn as_option_name(obj: Option<&Object>) -> Result<Option<String>, Error> {
    Ok(obj
        .map(|obj| obj.as_name_str())
        .transpose()?
        .map(|s| s.to_owned()))
}

pub(crate) fn as_name(obj: Option<&Object>) -> Result<String, Error> {
    as_option_name(obj)?.ok_or(Error::LoPdfError(lopdf::Error::DictKey))
}

pub(crate) fn as_option_byte_string(obj: Option<&Object>) -> Result<Option<Vec<u8>>, Error> {
    Ok(obj
        .map(|obj| obj.as_str())
        .transpose()?
        .map(|s| s.to_owned()))
}

pub(crate) fn as_byte_string(obj: Option<&Object>) -> Result<Vec<u8>, Error> {
    as_option_byte_string(obj)?.ok_or(Error::LoPdfError(lopdf::Error::DictKey))
}

pub(crate) fn as_option_integer(obj: Option<&Object>) -> Result<Option<i64>, Error> {
    Ok(obj.map(|obj| obj.as_i64()).transpose()?)
}

// pub(crate) fn as_integer(obj: Option<&Object>) -> Result<i64, InternalError> {
//     Ok(as_option_integer(obj)?.ok_or(InternalError::new(
//         "Key is missing.",
//         ApiErrorKind::BadRequest,
//         InternalErrorCodes::Default,
//     ))?)
// }

pub(crate) fn as_option_text_string(obj: Option<&Object>) -> Result<Option<String>, Error> {
    let byte_string = as_option_byte_string(obj)?;
    let text_string = byte_string
        .map(String::from_utf8)
        .transpose()
        .map_err(lopdf::Error::from)?;
    Ok(text_string)
}

pub(crate) fn as_array_or_byte_string(obj: Option<&Object>) -> Result<Vec<Vec<u8>>, Error> {
    let obj = obj.ok_or(Error::LoPdfError(lopdf::Error::DictKey))?;
    match obj {
        Object::String(string, _) => Ok(vec![string.to_owned()]),
        Object::Array(list) => {
            let mut result = Vec::new();
            for item in list {
                result.push(as_byte_string(Some(item))?);
            }
            Ok(result)
        }
        _ => Err(Error::LoPdfError(lopdf::Error::Type)),
    }
}

pub(crate) fn as_byte_range(obj: Option<&Object>) -> Result<Vec<(u64, u64)>, Error> {
    let mut result = Vec::new();
    let obj = obj.ok_or(Error::LoPdfError(lopdf::Error::DictKey))?;
    let list = obj.as_array()?;
    // Temporary store the value of prev loop so we can create pairs.
    let mut temp_item = None;
    for item in list {
        if let Some(prev_item) = temp_item {
            // Create tuple
            result.push((prev_item, u64::try_from(item.as_i64()?)?));
            // Reset `temp_item`
            temp_item = None;
        } else {
            // Store for next iteration
            temp_item = Some(u64::try_from(item.as_i64()?)?);
        }
    }
    if temp_item.is_some() {
        log::warn!("Expected pairs, got an uneven length.");
    }
    Ok(result)
}
