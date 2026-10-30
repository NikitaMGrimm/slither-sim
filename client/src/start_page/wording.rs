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

/// Static source login text and authored accessibility wording.
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
    /// Authored accessible name; the source image link has no alt/title text.
    pub change_skin: &'static str,
    /// Authored accessible name; the source image link has no alt/title text.
    pub choose_server: &'static str,
    /// Authored accessible name; the source image link has no alt/title text.
    pub graphics_quality: &'static str,
    /// Authored high-quality value for accessibility, independent of Debug output.
    pub quality_high: &'static str,
    /// Authored low-quality value for accessibility, independent of Debug output.
    pub quality_low: &'static str,
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

    /// Source text: 06-login-and-hud.js:254-293, 20a-game-over-and-codes.js:11-31.
    /// Accessibility names/values are authored: site/index.html:288-301,347-350
    /// provides image links without alt/title text. See docs/start-menu.md.
    pub fn wording(self) -> MenuWording {
        match self {
            Self::English => MenuWording {
                change_skin: "Change Skin",
                choose_server: "Choose Server",
                graphics_quality: "Graphics quality",
                quality_high: "High",
                quality_low: "Low",
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
                change_skin: "Skin ändern",
                choose_server: "Server wählen",
                graphics_quality: "Grafikqualität",
                quality_high: "Hoch",
                quality_low: "Niedrig",
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
                change_skin: "Changer d'apparence",
                choose_server: "Choisir un serveur",
                graphics_quality: "Qualité graphique",
                quality_high: "Élevée",
                quality_low: "Faible",
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
                change_skin: "Mudar aparência",
                choose_server: "Escolher servidor",
                graphics_quality: "Qualidade gráfica",
                quality_high: "Alta",
                quality_low: "Baixa",
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
