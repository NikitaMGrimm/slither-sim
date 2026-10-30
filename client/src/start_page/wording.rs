//! Source labels and hints, kept as plain text rather than HTML.

/// The four locales present in the inspected source login.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuLocale {
    /// English (also the fallback).
    #[default]
    English,
    /// German.
    German,
    /// French.
    French,
    /// Portuguese.
    Portuguese,
}

/// Static translated login text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuWording {
    /// Initial Play label, without measurement padding.
    pub play: &'static str,
    /// Label after any accepted game over.
    pub play_again: &'static str,
    /// Nickname input placeholder.
    pub nickname: &'static str,
    /// Last-length prefix, rendered separately from its bold number.
    pub final_length: &'static str,
    /// Three tips in source order.
    pub tips: [&'static str; 3],
}

impl MenuLocale {
    /// Select by the source's exact language code; unsupported codes use English.
    pub fn from_code(code: &str) -> Self {
        match code {
            "de" => Self::German,
            "fr" => Self::French,
            "pt" => Self::Portuguese,
            _ => Self::English,
        }
    }

    /// Text specified by 06-login-and-hud.js:254-293 and 20a-game-over-and-codes.js:11-31.
    pub fn wording(self) -> MenuWording {
        match self {
            Self::English => MenuWording {
                play: "Play",
                play_again: "Play Again",
                nickname: "Nickname",
                final_length: "Your final length was",
                tips: [
                    "Eat to grow longer!",
                    "Don't run into other players!",
                    "When longer, hold the mouse for a speed boost!",
                ],
            },
            Self::German => MenuWording {
                play: "Spielen",
                play_again: "Play Again",
                nickname: "Nickname",
                final_length: "Deine endgültige Länge war",
                tips: [
                    "Esse um zu wachsen!",
                    "Klicke für mehr Geschwindigkeit!",
                    "Bewege dich nicht in andere Schlangen!",
                ],
            },
            Self::French => MenuWording {
                play: "Jouer",
                play_again: "Jouer",
                nickname: "Surnom",
                final_length: "Votre longueur finale était de",
                tips: [
                    "Mange pour croître !",
                    "Clique pour courir !",
                    "Ne laissez pas votre tête toucher d'autres serpents !",
                ],
            },
            Self::Portuguese => MenuWording {
                play: "Joga",
                play_again: "Joga",
                nickname: "Apelido",
                final_length: "Seu comprimento final foi de",
                tips: [
                    "Coma para crescer!",
                    "Clique para correr!",
                    "Não deixe que sua cabeça para tocar outras cobras!",
                ],
            },
        }
    }
}

/// Normalize individual UTF-16 units before truncation; DEL is accepted.
/// Surrogate pairs (and lone surrogates from a browser) each become two/one spaces.
pub fn normalize_nickname_units(units: &[u16]) -> String {
    units
        .iter()
        .take(24)
        .map(|unit| {
            if (32..=127).contains(unit) {
                char::from(*unit as u8)
            } else {
                ' '
            }
        })
        .collect()
}

/// Convenience seam for native committed Unicode text.
pub fn normalize_nickname(text: &str) -> String {
    normalize_nickname_units(&text.encode_utf16().take(24).collect::<Vec<_>>())
}
