use cryptographic_message_syntax::SignerBuilder;
use pdf_signing::{PDFSigningDocument, UserSignatureInfo};
use std::{fs::File, io::Write};
use x509_certificate::{CapturedX509Certificate, InMemorySigningKeyPair};

fn main() {
    // let pdf_file_name = "test-small-1sig.pdf";
    let pdf_file_name = "test-small-3sig.pdf";
    let pdf_data = std::fs::read(format!("./examples/assets/{}", pdf_file_name)).unwrap();

    // Use Cert/Private key to sign data
    // To create cert, see `Create_Cert.md` file.
    let cert = std::fs::read_to_string("./examples/assets/pdf_cert.crt").unwrap();
    let x509_cert = CapturedX509Certificate::from_pem(cert).unwrap();
    let private_key_data = std::fs::read_to_string("./examples/assets/pkcs8.pem").unwrap();
    let private_key = InMemorySigningKeyPair::from_pkcs8_pem(&private_key_data).unwrap();
    let signer = SignerBuilder::new(&private_key, x509_cert);
    // Try using a time server. If it fails we continue without it.
    // Alternative time servers:
    // 1: https://freetsa.org/tsr
    // 2: http://timestamp.digicert.com
    // let signer_fallback = signer.clone();
    // let signer = signer
    //     .time_stamp_url("http://timestamp.digicert.com")
    //     .or::<reqwest::Error>(Ok(signer_fallback))
    //     .expect("Can not happen because of fallback.");

    let users_signature_info = vec![
        UserSignatureInfo {
            user_id: "9".to_owned(),
            user_name: "Alice".to_owned(),
            user_email: "alice@test.com".to_owned(),
            user_signature: std::fs::read("./examples/assets/sig1.png").unwrap(),
            user_signing_keys: signer.clone(),
        },
        UserSignatureInfo {
            user_id: "256".to_owned(),
            user_name: "Bob".to_owned(),
            user_email: "bob@test.com".to_owned(),
            user_signature: std::fs::read("./examples/assets/sig2.png").unwrap(),
            user_signing_keys: signer.clone(),
        },
        UserSignatureInfo {
            user_id: "272".to_owned(),
            user_name: "Charlie".to_owned(),
            user_email: "charlie@test.com".to_owned(),
            user_signature: std::fs::read("./examples/assets/sig1.png").unwrap(),
            user_signing_keys: signer.clone(),
        },
        UserSignatureInfo {
            user_id: "292".to_owned(),
            user_name: "Dave".to_owned(),
            user_email: "dave@test.com".to_owned(),
            user_signature: std::fs::read("./examples/assets/sig3.png").unwrap(),
            user_signing_keys: signer.clone(),
        },
        UserSignatureInfo {
            user_id: "274".to_owned(),
            user_name: "Ester".to_owned(),
            user_email: "ester@test.com".to_owned(),
            user_signature: std::fs::read("./examples/assets/sig2.png").unwrap(),
            user_signing_keys: signer.clone(),
        },
    ];

    let mut pdf_signing_document =
        PDFSigningDocument::read_from(&*pdf_data, pdf_file_name.to_owned()).unwrap();
    let pdf_file_data = pdf_signing_document
        .sign_document(users_signature_info)
        .unwrap();

    let mut pdf_file = File::create("./examples/result.pdf").unwrap();
    pdf_file.write_all(&pdf_file_data).unwrap();
}
