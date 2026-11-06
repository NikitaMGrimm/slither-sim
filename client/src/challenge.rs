//! A tokenized, closed grammar for the string-copy challenge family.
//!
//! Production login keeps a stock-shaped acknowledgement for unrecognized
//! challenges. No server-supplied source is evaluated or interpreted generally.

use std::fmt;

/// Maximum decoded source size accepted by the prototype.
pub const MAX_SOURCE_BYTES: usize = 256;

/// A challenge was outside the supported, bounded grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChallengeError {
    /// Empty, odd-length, or oversized encoded input.
    Size,
    /// Challenge text would exceed the enclosing frame's byte bound.
    FrameSize,
    /// The wire encoding contained a non-ASCII letter.
    Alphabet,
    /// A decoded nibble exceeded fifteen.
    Nibble,
    /// A token, expression, or trailing input was unsupported.
    Syntax,
    /// A literal was not unescaped printable ASCII, or the literals did not
    /// match the copy count.
    Literal,
}

impl fmt::Display for ChallengeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Size => "challenge size must be even and within 2..=512 bytes",
            Self::FrameSize => "challenge text exceeds the frame byte bound",
            Self::Alphabet => "challenge text contains an unsupported byte",
            Self::Nibble => "challenge encoding contains a nibble above 15",
            Self::Syntax => "unsupported challenge syntax or trailing tokens",
            Self::Literal => {
                "challenge literals must be unescaped printable ASCII matching the copy count"
            }
        })
    }
}

impl std::error::Error for ChallengeError {}

/// Answer recognized copy challenges exactly, or attempt login with an initialized
/// acknowledgement. The stock client initializes its reply before handling server
/// text; community servers can leave it untouched or ignore it entirely. An
/// unsupported program is therefore not itself a reason to disconnect.
///
/// This is a best-effort fallback, not an interpreter for arbitrary JavaScript.
/// The server still decides whether to accept the ordinary login. Preserve the
/// stock text-byte admission and the enclosing frame's existing byte bound.
pub fn answer_challenge(encoded: &[u8]) -> Result<Vec<u8>, ChallengeError> {
    if encoded.len() >= crate::protocol::MAX_FRAME_BYTES {
        return Err(ChallengeError::FrameSize);
    }
    if !encoded.iter().all(|byte| (b'A'..=b'z').contains(byte)) {
        return Err(ChallengeError::Alphabet);
    }
    Ok(solve_challenge(encoded).unwrap_or_else(|_| {
        // No secret or authentication proof is available in this path. Use a
        // deterministic 27-byte seed so native and browser adapters agree.
        let mut reply = vec![b'A'; 27];
        rotate_reply(&mut reply);
        reply
    }))
}

/// Decode a challenge command payload (without its command byte) and solve it.
///
/// Only a declaration, literal appends, and one copy loop over every literal
/// byte are accepted, with literals in double or single quotes and an optional
/// braced loop body. Official servers send a 5- and a 22-byte literal copied by
/// a 27-byte loop. Some community servers, like slither-sim's legacy path,
/// send one single-quoted 24-byte literal and accept the rotated 24-byte reply
/// (see docs/raw-server.md, "Packet compatibility").
/// Whitespace between tokens is allowed. Literal contents are runtime input,
/// not credentials embedded in the library.
pub fn solve_challenge(encoded: &[u8]) -> Result<Vec<u8>, ChallengeError> {
    if encoded.is_empty()
        || encoded.len() > MAX_SOURCE_BYTES * 2
        || !encoded.len().is_multiple_of(2)
    {
        return Err(ChallengeError::Size);
    }
    let mut source = Vec::with_capacity(encoded.len() / 2);
    let mut key = 23_u8;
    let mut high = 0;
    for (index, &letter) in encoded.iter().enumerate() {
        if !letter.is_ascii_alphabetic() {
            return Err(ChallengeError::Alphabet);
        }
        let nibble = (letter.to_ascii_lowercase() - b'a' + 26 - key) % 26;
        if nibble > 15 {
            return Err(ChallengeError::Nibble);
        }
        if index % 2 == 0 {
            high = nibble << 4;
        } else {
            source.push(high | nibble);
        }
        key = (key + 17) % 26;
    }
    let mut parser = Parser {
        source: &source,
        offset: 0,
    };
    parser.tokens(&["var", "a", "="])?;
    let mut reply = parser.literal()?.to_vec();
    parser.tokens(&[";"])?;
    loop {
        let start = parser.offset;
        if parser.tokens(&["a", "+="]).is_err() {
            parser.offset = start;
            break;
        }
        reply.extend(parser.literal()?);
        parser.tokens(&[";"])?;
    }
    parser.tokens(&["for", "(", "var", "i", "=", "0", ";", "i", "<"])?;
    if parser.count()? != reply.len() {
        return Err(ChallengeError::Literal);
    }
    parser.tokens(&[";", "i", "++", ")"])?;
    parser.whitespace();
    let braced = parser.source.get(parser.offset) == Some(&b'{');
    if braced {
        parser.tokens(&["{"])?;
    }
    parser.tokens(&[
        "idba",
        "[",
        "i",
        "]",
        "=",
        "a",
        ".",
        "charCodeAt",
        "(",
        "i",
        ")",
        ";",
    ])?;
    if braced {
        parser.tokens(&["}"])?;
    }
    parser.whitespace();
    if parser.offset != source.len() {
        return Err(ChallengeError::Syntax);
    }

    rotate_reply(&mut reply);
    Ok(reply)
}

fn rotate_reply(reply: &mut [u8]) {
    let mut rotation = 0_i32;
    for (index, byte) in reply.iter_mut().enumerate() {
        let (base, upper) = if *byte >= b'a' {
            (i32::from(b'a'), i32::from(*byte) - 32)
        } else {
            (i32::from(b'A'), i32::from(*byte))
        };
        let value = upper - i32::from(b'A');
        if index == 0 {
            rotation = 3 + value;
        }
        // JavaScript's remainder preserves the dividend's sign, including when
        // punctuation makes it negative. Euclidean modulo would change replies.
        *byte = ((value + rotation) % 26 + base) as u8;
        rotation += 4 + value;
    }
}

struct Parser<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn whitespace(&mut self) {
        while self
            .source
            .get(self.offset)
            .is_some_and(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.offset += 1;
        }
    }

    fn tokens(&mut self, expected: &[&str]) -> Result<(), ChallengeError> {
        for &token in expected {
            self.whitespace();
            let start = self.offset;
            let Some(&byte) = self.source.get(start) else {
                return Err(ChallengeError::Syntax);
            };
            if byte.is_ascii_alphabetic() || byte == b'_' {
                self.offset += 1;
                while self
                    .source
                    .get(self.offset)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                {
                    self.offset += 1;
                }
            } else if byte.is_ascii_digit() {
                self.offset += 1;
                while self.source.get(self.offset).is_some_and(u8::is_ascii_digit) {
                    self.offset += 1;
                }
            } else if self
                .source
                .get(start..start + 2)
                .is_some_and(|s| s == b"+=" || s == b"++")
            {
                self.offset += 2;
            } else {
                self.offset += 1;
            }
            if &self.source[start..self.offset] != token.as_bytes() {
                return Err(ChallengeError::Syntax);
            }
        }
        Ok(())
    }

    fn literal(&mut self) -> Result<&'a [u8], ChallengeError> {
        self.whitespace();
        let quote = match self.source.get(self.offset) {
            Some(&quote @ (b'"' | b'\'')) => quote,
            _ => return Err(ChallengeError::Literal),
        };
        let start = self.offset + 1;
        let length = self.source[start..]
            .iter()
            .position(|&c| c == quote)
            .ok_or(ChallengeError::Literal)?;
        let literal = &self.source[start..start + length];
        if literal.is_empty()
            || literal
                .iter()
                .any(|c| !(b' '..=b'~').contains(c) || *c == b'\\')
        {
            return Err(ChallengeError::Literal);
        }
        self.offset = start + length + 1;
        Ok(literal)
    }

    /// A decimal copy count without leading zeros.
    fn count(&mut self) -> Result<usize, ChallengeError> {
        self.whitespace();
        let start = self.offset;
        while self.source.get(self.offset).is_some_and(u8::is_ascii_digit) {
            self.offset += 1;
        }
        let digits = &self.source[start..self.offset];
        if digits.is_empty() || digits[0] == b'0' || digits.len() > 3 {
            return Err(ChallengeError::Syntax);
        }
        Ok(digits
            .iter()
            .fold(0, |count, digit| count * 10 + usize::from(digit - b'0')))
    }
}
