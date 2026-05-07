use hmac::Mac;
use hmac::digest::KeyInit;
use sha2::Sha256;

type HmacSha256 = hmac::Hmac<Sha256>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum HmacError {
    InvalidKey,
}

pub(crate) fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, HmacError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| HmacError::InvalidKey)?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}
