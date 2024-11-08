# Create a Self signed Cert for Document Signing

Commands needed to create a PDF signing certificate.

info: https://www.adobe.com/devnet-docs/acrobatetk/tools/DigSig/changes.html

```bash
# Create Cert and Private key
openssl req \
  -newkey rsa:4096 -x509 -sha256 \
  -days 365 -nodes \
  -out pdf_cert.crt -keyout pdf_cert_private.key \
  -addext extendedKeyUsage=1.3.6.1.4.1.311.80.1,1.2.840.113583.1.1.5 \
  -addext keyUsage=digitalSignature,keyAgreement

# Create PKCS8 cert (contains private key)
openssl pkcs8 -topk8 -outform PEM -in pdf_cert_private.key -out pkcs8.pem -nocrypt
# Public key only (only needed for debugging)
openssl x509 -pubkey -noout -in pdf_cert.crt > pdf_cert_pubic_key.pem
```

Inspect certificate:

```bash
openssl cms -inform DER -in signature.der -cmsout -print
```

Verify CMS:

```bash
openssl cms -verify -binary -verify -in signature.der -content result-no-contents.pdf -CAfile pdf_cert.crt -inform DER -out validation_output -noverify
```

Verify signerInfos-signature:

```bash
# Sign
openssl dgst -sha256 -sign pdf_cert_private.key -out signerInfos-signature_openssl.bin -in signed_content.der
# Validate
openssl dgst -sha256 -verify pdf_cert_pubic_key.pem -keyform PEM -signature signerInfos-signature_openssl.bin signed_content.der
```

