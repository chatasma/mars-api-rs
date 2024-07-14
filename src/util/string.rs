use std::io::Write;

use flate2::{write::ZlibEncoder, Compression};

pub fn to_utf8_byte_array(text: &String) -> &[u8] {
    text.as_bytes()
}

pub fn deflate_string(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    // https://github.com/madler/zlib/blob/master/zlib.h#L239
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn enumify(target: &str) -> String {
    target.trim().to_uppercase().replace(" ", "_")
}