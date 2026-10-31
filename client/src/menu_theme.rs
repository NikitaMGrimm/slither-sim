//! Typed site-style roles. Motion and placement never enter raster keys.
use crate::presentation::{TextFamily, TextSlant, TextStyle, TextWeight};

/// Roles shared by menu descriptions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlStyle {
    /// Main green action.
    PrimaryButton,
    /// Native login Play: admission disables input without dimming its return fade.
    LoginPlay,
    /// Native nickname: fitted text and undimmed disabled surface.
    Nickname,
    /// Existing quality link, with only a keyboard focus surface.
    QualityButton,
    /// Purple action.
    SecondaryButton,
    /// Single-line purple editor.
    Field,
    /// Bold panel heading.
    Heading,
    /// Muted instructions.
    HelpText,
    /// Purple panel background.
    Surface,
    /// Selection indication.
    SelectedCell,
}
/// Authored raster recipes already used by the native login.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SurfaceRecipe {
    /// Green/purple action capsule and its shadow.
    Button,
    /// Pointer/focus overlay without an extra shadow.
    Overlay,
    /// Purple bordered field with its existing shadow.
    Field,
    /// Existing compact link/focus capsule.
    Compact,
}
/// Sampled control layer whose raster content is cached independently of opacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlState {
    /// Base surface.
    Idle,
    /// Hover/focus layer.
    Hovered,
    /// Pressed layer.
    Pressed,
}
/// Raster-affecting surface tokens; also the complete material cache identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SurfaceTokens {
    /// Top/bottom ink.
    pub colors: [[u8; 3]; 2],
    /// Logical corner radius.
    pub radius: u16,
    /// Existing material recipe: primary, overlay, field or compact.
    pub recipe: SurfaceRecipe,
}
/// Initial theme, seeded from the existing native login and browser overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuTheme {
    /// Pale lavender ink.
    pub text: [u8; 3],
    /// Muted field placeholder.
    pub placeholder: [u8; 3],
    /// Selected text fill.
    pub selection: [u8; 3],
    /// Purple surface.
    pub field: [u8; 3],
    /// Per-core-step hover damping.
    pub hover_rate: f64,
    /// Per-core-step press damping.
    pub press_rate: f64,
    /// Per-callback hover/press release, shared with existing login damping.
    pub release_rate: f64,
    /// Disabled control opacity.
    pub disabled_alpha: f64,
}
impl Default for MenuTheme {
    fn default() -> Self {
        Self {
            text: [224, 224, 255],
            placeholder: [112, 112, 176],
            selection: [150, 108, 213],
            field: [76, 68, 124],
            hover_rate: 0.33,
            press_rate: 0.33,
            release_rate: 0.2,
            disabled_alpha: 0.38,
        }
    }
}
impl MenuTheme {
    /// Font raster tokens. Native Lucida bold currently resolves to Arial Bold.
    pub fn text_style(self, role: ControlStyle) -> TextStyle {
        use ControlStyle::*;
        TextStyle {
            size: match role {
                Heading => 24.,
                HelpText => 18.,
                PrimaryButton | LoginPlay | SecondaryButton => 20.,
                QualityButton => 14.,
                _ => 15.,
            },
            weight: if matches!(role, Heading | PrimaryButton | LoginPlay | SecondaryButton) {
                TextWeight::Bold
            } else {
                TextWeight::Normal
            },
            family: if matches!(
                role,
                PrimaryButton | LoginPlay | SecondaryButton | QualityButton
            ) {
                TextFamily::Arial
            } else {
                TextFamily::Lucida
            },
            slant: TextSlant::Normal,
            color: if matches!(role, PrimaryButton | LoginPlay | SecondaryButton) {
                [255; 3]
            } else {
                self.text
            },
        }
    }
    /// Login's existing admission gate leaves field/link paint undimmed.
    pub fn disabled_opacity(self, role: ControlStyle) -> f64 {
        if matches!(
            role,
            ControlStyle::Nickname | ControlStyle::LoginPlay | ControlStyle::QualityButton
        ) {
            1.
        } else {
            self.disabled_alpha
        }
    }
    /// Complete raster identity for a surface and its pointer overlay.
    pub fn surface(self, role: ControlStyle, state: ControlState) -> SurfaceTokens {
        use ControlStyle::*;
        if state == ControlState::Hovered {
            return SurfaceTokens {
                colors: [[117, 240, 202], [72, 171, 132]],
                radius: 24,
                recipe: SurfaceRecipe::Overlay,
            };
        }
        if state == ControlState::Pressed {
            return SurfaceTokens {
                colors: [[12, 20, 18], [40, 59, 54]],
                radius: 24,
                recipe: SurfaceRecipe::Overlay,
            };
        }
        SurfaceTokens {
            colors: match role {
                PrimaryButton | LoginPlay => [[86, 172, 129], [54, 108, 81]],
                SecondaryButton => [[150, 108, 213], [100, 70, 150]],
                _ => [self.field; 2],
            },
            radius: if role == QualityButton { 44 } else { 24 },
            recipe: if matches!(role, Field | Nickname | Surface) {
                SurfaceRecipe::Field
            } else if role == QualityButton {
                SurfaceRecipe::Compact
            } else {
                SurfaceRecipe::Button
            },
        }
    }
}
