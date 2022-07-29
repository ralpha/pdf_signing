use cryptographic_message_syntax::SignerBuilder;
use serde::{Deserialize, Serialize};

/// The info provided to PDF service when a document needs to be signed.
#[derive(Clone)]
pub struct UserSignatureInfo<'a> {
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub user_signature: Vec<u8>,
    pub user_signing_keys: SignerBuilder<'a>,
}

/// The info inside the PDF form signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFormSignatureInfo {
    pub user_id: String,
}
