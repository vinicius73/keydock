use percent_encoding::percent_decode_str;

pub(crate) fn decode_to_bytes(raw: &str) -> Vec<u8> {
    percent_decode_str(raw).collect::<Vec<u8>>()
}
