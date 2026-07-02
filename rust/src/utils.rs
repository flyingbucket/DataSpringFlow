use crate::merkle::HashRes;

pub fn hashres_to_hex(bytes: HashRes) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);

    for &b in &bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }

    out
}
