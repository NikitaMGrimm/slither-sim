//! Restricted protocol-19 challenge adapter. Source is never evaluated.
use slither_protocol::{
    ErrorKind,
    challenge::{self, Grammar},
};

/// Solve an encoded `6` command payload, excluding its command byte.
/// Only the observed exact two-literal template is admitted.
pub fn solve_challenge(encoded: &[u8]) -> Result<Vec<u8>, String> {
    challenge::stock(encoded, Grammar::ExactTemplate)
        .map(|reply| reply.to_vec())
        .map_err(|error| legacy_error(encoded, error.kind))
}

fn legacy_error(encoded: &[u8], kind: ErrorKind) -> String {
    if kind == ErrorKind::Syntax {
        // The old solver checks each literal before later punctuation. Probe
        // only those literals through the shared decoder to preserve its error
        // priority for multiply malformed sources, without another cipher/parser.
        const TEMPLATE: &[u8; 184] = b"eulxugxmjvncwxdslcvjdslbtkbthyqpbnffopvmfynhvqdzlitrbajjrszbiuqdymgvoewnewmfuocxkgtfxogfrizziulatqgwnzwizolfqrymgfrlwweumhutfzjjrrzxmhtnbqjwrdyvhbonuvfrjmvjdwlxugzpjjrgzpftqetqfzjhsreb";
        let mut probe = *TEMPLATE;
        for (start, end, guard) in [(18, 28, 0..18), (40, 84, 28..40)] {
            if encoded.len() < end || !encoded[guard.clone()].eq_ignore_ascii_case(&TEMPLATE[guard])
            {
                break;
            }
            probe[start..end].copy_from_slice(&encoded[start..end]);
            if challenge::literal_27(&probe, Grammar::ExactTemplate)
                .is_err_and(|error| error.kind == ErrorKind::Literal)
            {
                return "Unsupported challenge literal".into();
            }
        }
    }
    match kind {
        ErrorKind::InvalidLength => "Invalid challenge size",
        ErrorKind::Alphabet => "Invalid challenge encoding",
        ErrorKind::Nibble => "Invalid challenge nibble",
        ErrorKind::Literal => "Unsupported challenge literal",
        _ => "Unsupported challenge syntax",
    }
    .into()
}
