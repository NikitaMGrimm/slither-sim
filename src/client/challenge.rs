//! Restricted protocol-19 challenge solver. Captured JavaScript is parsed as a
//! fixed string-copy template, never evaluated or passed to another process.

const MAX_SOURCE_BYTES: usize = 256;
const PREFIX: &[u8] = b"var a = \"";
const BETWEEN: &[u8] = b"\";a+=\"";
const SUFFIX: &[u8] = b"\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}";

/// Solve an encoded `6` command payload, excluding its command byte.
///
/// Only the observed two-literal template (5 + 22 printable ASCII bytes) is
/// supported. Unknown syntax fails closed; this is not a JavaScript interpreter.
pub fn solve_challenge(encoded: &[u8]) -> Result<Vec<u8>, String> {
    if encoded.is_empty()
        || encoded.len() > MAX_SOURCE_BYTES * 2
        || !encoded.len().is_multiple_of(2)
    {
        return Err("Invalid challenge size".into());
    }
    let mut source = Vec::with_capacity(encoded.len() / 2);
    let mut key = 23_u8;
    let mut byte = 0_u8;
    for (index, &letter) in encoded.iter().enumerate() {
        if !letter.is_ascii_alphabetic() {
            return Err("Invalid challenge encoding".into());
        }
        let nibble = (letter.to_ascii_lowercase() - b'a' + 26 - key) % 26;
        if nibble > 15 {
            return Err("Invalid challenge nibble".into());
        }
        byte = byte * 16 + nibble;
        key = (key + 17) % 26;
        if index % 2 == 1 {
            source.push(byte);
            byte = 0;
        }
    }

    let mut cursor = source
        .strip_prefix(PREFIX)
        .ok_or("Unsupported challenge syntax")?;
    let mut reply = [0_u8; 27];
    copy_literal(&mut cursor, &mut reply[..5])?;
    cursor = cursor
        .strip_prefix(BETWEEN)
        .ok_or("Unsupported challenge syntax")?;
    copy_literal(&mut cursor, &mut reply[5..])?;
    if cursor != SUFFIX {
        return Err("Unsupported challenge syntax".into());
    }

    rotate_reply(&mut reply, 4);
    Ok(reply.to_vec())
}

pub(super) fn rotate_reply(reply: &mut [u8], step: i32) {
    let mut rotation = 0_i32;
    for (index, byte) in reply.iter_mut().enumerate() {
        let mut base = i32::from(b'A');
        let mut value = i32::from(*byte);
        if value >= i32::from(b'a') {
            base += 32;
            value -= 32;
        }
        value -= i32::from(b'A');
        if index == 0 {
            rotation = 3 + value;
        }
        // JS remainder keeps the dividend's sign. Punctuation can make it
        // negative; Euclidean modulo changes real captured handshake replies.
        *byte = ((value + rotation) % 26 + base) as u8;
        rotation += step + value;
    }
}

fn copy_literal(cursor: &mut &[u8], output: &mut [u8]) -> Result<(), String> {
    let literal = cursor
        .get(..output.len())
        .ok_or("Unsupported challenge syntax")?;
    if literal
        .iter()
        .any(|&byte| !(b' '..=b'~').contains(&byte) || byte == b'"' || byte == b'\\')
    {
        return Err("Unsupported challenge literal".into());
    }
    output.copy_from_slice(literal);
    *cursor = &cursor[output.len()..];
    Ok(())
}
