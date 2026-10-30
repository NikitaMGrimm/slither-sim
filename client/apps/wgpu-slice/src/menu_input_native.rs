//! Windows winit 0.30.13 translation and injectable host capabilities.
//! Other native platforms need their own IME rule before enabling this editor.

use slither_client::menu_input::{
    EditIntent, GameplayKey, HostEditEffect, MenuInputState, Movement,
};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{ElementState, Ime, KeyEvent, WindowEvent},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
    window::{ImePurpose, Window},
};

/// The public fields used from KeyEvent. winit's private platform payload prevents
/// constructing KeyEvent in offline tests; both production and synthetic events
/// use this exact translation seam without unsafe fabricated platform data.
#[derive(Clone, Debug)]
pub struct KeyInput<'a> {
    pub logical: &'a Key,
    pub physical: PhysicalKey,
    pub text: Option<&'a str>,
    pub state: ElementState,
    pub repeat: bool,
}
impl<'a> From<&'a KeyEvent> for KeyInput<'a> {
    fn from(event: &'a KeyEvent) -> Self {
        Self {
            logical: &event.logical_key,
            physical: event.physical_key,
            text: event.text.as_deref(),
            state: event.state,
            repeat: event.repeat,
        }
    }
}

/// Windows input policy and modifier bookkeeping; created when a menu is shown.
#[derive(Default)]
pub struct NativeMenuInput {
    pub state: MenuInputState,
    modifiers: ModifiersState,
}
/// Copied result. Consumed keyboard events must not reach Play/Stop/gameplay again.
#[derive(Default)]
pub struct NativeInputResult {
    pub consumed: bool,
    pub effects: Vec<HostEditEffect>,
}

impl NativeMenuInput {
    /// Update modifiers explicitly (also used by the synthetic event tests).
    pub fn modifiers(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    /// Map the fields of a winit KeyEvent. Logical keys own editing shortcuts;
    /// physical keys own gameplay. Ctrl+Alt text is retained for Windows AltGr.
    pub fn key(&mut self, event: KeyInput<'_>) -> NativeInputResult {
        let pressed = event.state == ElementState::Pressed;
        let editing = self.state.suspends_gameplay();
        let gameplay = match event.physical {
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(GameplayKey::Left),
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(GameplayKey::Right),
            PhysicalKey::Code(KeyCode::ArrowUp) => Some(GameplayKey::Up),
            PhysicalKey::Code(KeyCode::Space) => Some(GameplayKey::Space),
            _ => None,
        };
        let mut intents = Vec::new();
        let process = matches!(event.logical, Key::Named(NamedKey::Process));
        if let Some(key) = gameplay.filter(|_| !pressed || !process) {
            intents.push(EditIntent::Gameplay { key, pressed });
        }
        // winit 0.30.13 src/platform_impl/windows/keyboard_layout.rs:946 maps
        // VK_PROCESSKEY to Process. On Windows KeyEvent.text can still carry a
        // payload for Process; reject it regardless, before text/shortcut mapping.
        // Still accept a physical Enter key-up, including a Process key-up, to
        // release the composition Enter barrier without activating anything.
        let physical_enter = matches!(
            event.physical,
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
        );
        let enter_release = !pressed && physical_enter;
        let composing = self
            .state
            .focus()
            .is_some_and(|field| self.state.editor(field).composing());
        let ime_enter_press = pressed && physical_enter && (process || composing);
        if enter_release {
            intents.push(EditIntent::Enter {
                pressed: false,
                repeat: false,
            });
        } else if ime_enter_press {
            intents.push(EditIntent::ImeEnterPressed);
        }
        if process || enter_release || ime_enter_press {
            return NativeInputResult {
                consumed: process || ime_enter_press || editing,
                effects: intents
                    .into_iter()
                    .flat_map(|intent| self.state.apply(intent))
                    .collect(),
            };
        }
        let control = self.modifiers.control_key() && !self.modifiers.alt_key();
        let shift = self.modifiers.shift_key();
        if pressed && matches!(event.logical, Key::Named(NamedKey::Enter)) {
            intents.push(EditIntent::Enter {
                pressed,
                repeat: event.repeat,
            });
        } else if pressed && editing {
            let intent = match event.logical {
                Key::Named(NamedKey::ArrowLeft) => Some(EditIntent::Move {
                    movement: if control {
                        Movement::WordLeft
                    } else {
                        Movement::Left
                    },
                    extend: shift,
                }),
                Key::Named(NamedKey::ArrowRight) => Some(EditIntent::Move {
                    movement: if control {
                        Movement::WordRight
                    } else {
                        Movement::Right
                    },
                    extend: shift,
                }),
                Key::Named(NamedKey::Home) => Some(EditIntent::Move {
                    movement: Movement::Home,
                    extend: shift,
                }),
                Key::Named(NamedKey::End) => Some(EditIntent::Move {
                    movement: Movement::End,
                    extend: shift,
                }),
                Key::Named(NamedKey::Backspace) => Some(EditIntent::Backspace),
                Key::Named(NamedKey::Delete) => Some(EditIntent::Delete),
                Key::Named(NamedKey::Escape) => Some(EditIntent::CancelComposition),
                Key::Character(c) if control => match c.to_lowercase().as_str() {
                    "a" => Some(EditIntent::SelectAll),
                    "c" => Some(EditIntent::Copy),
                    "x" => Some(EditIntent::Cut),
                    "v" => Some(EditIntent::Paste),
                    "z" => Some(if shift {
                        EditIntent::Redo
                    } else {
                        EditIntent::Undo
                    }),
                    "y" => Some(EditIntent::Redo),
                    _ => None,
                },
                _ => None,
            };
            if let Some(intent) = intent {
                intents.push(intent);
            } else if !control
                && !self.modifiers.super_key()
                && let Some(text) = event.text
            {
                // Named control text such as Tab/Enter is not a single-line edit.
                if !text.chars().any(char::is_control) {
                    intents.push(EditIntent::KeyText(text.into()));
                }
            }
        }
        NativeInputResult {
            consumed: editing || gameplay.is_some(),
            effects: intents
                .into_iter()
                .flat_map(|intent| self.state.apply(intent))
                .collect(),
        }
    }

    /// Translate real events. This function also accepts authored Ime, Focused
    /// and ModifiersChanged WindowEvents without a window or event loop.
    pub fn window_event(&mut self, event: &WindowEvent) -> NativeInputResult {
        let intent = match event {
            WindowEvent::KeyboardInput { event, .. } => return self.key(event.into()),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers(modifiers.state());
                None
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.modifiers = ModifiersState::empty();
                }
                Some(EditIntent::WindowFocused(*focused))
            }
            WindowEvent::Ime(ime) => Some(match ime {
                Ime::Enabled => EditIntent::ImeEnabled(true),
                Ime::Disabled => EditIntent::ImeEnabled(false),
                Ime::Preedit(text, selection) => EditIntent::Preedit(text.clone(), *selection),
                Ime::Commit(text) => EditIntent::ImeCommit(text.clone()),
            }),
            _ => None,
        };
        NativeInputResult {
            consumed: matches!(event, WindowEvent::Ime(_)),
            effects: intent.map_or_else(Vec::new, |intent| self.state.apply(intent)),
        }
    }

    /// Execute host effects outside the reducer borrow, feeding clipboard results
    /// back through token validation. Return gameplay/activation/errors to owner.
    pub fn execute(
        &mut self,
        effects: Vec<HostEditEffect>,
        host: &mut impl EditHost,
    ) -> Vec<HostEditEffect> {
        let mut remaining = Vec::new();
        for effect in effects {
            let feedback = match effect {
                HostEditEffect::ImeAllowed(allowed) => {
                    host.ime_allowed(allowed);
                    None
                }
                HostEditEffect::ImePurposeNormal => {
                    host.ime_purpose_normal();
                    None
                }
                HostEditEffect::ImeCursorArea { position, size } => {
                    host.ime_cursor_area(position, size);
                    None
                }
                HostEditEffect::ReadClipboard(token) => Some(EditIntent::ClipboardRead {
                    token,
                    result: host.read_clipboard(),
                }),
                HostEditEffect::WriteClipboard { token, text } => {
                    Some(EditIntent::ClipboardWritten {
                        token,
                        result: host.write_clipboard(&text),
                    })
                }
                other => {
                    remaining.push(other);
                    None
                }
            };
            if let Some(feedback) = feedback {
                remaining.extend(self.state.apply(feedback));
            }
        }
        remaining
    }
}

/// Injectable OS capabilities. Tests never create a window or touch the clipboard.
pub trait EditHost {
    fn ime_allowed(&mut self, allowed: bool);
    fn ime_purpose_normal(&mut self);
    fn ime_cursor_area(&mut self, position: [f64; 2], size: [f64; 2]);
    fn read_clipboard(&mut self) -> Result<String, String>;
    fn write_clipboard(&mut self, text: &str) -> Result<(), String>;
}

/// Real native host, only used by an enabled menu hook.
pub struct WindowEditHost<'a>(pub &'a Window);
impl EditHost for WindowEditHost<'_> {
    fn ime_allowed(&mut self, allowed: bool) {
        self.0.set_ime_allowed(allowed);
    }
    fn ime_purpose_normal(&mut self) {
        self.0.set_ime_purpose(ImePurpose::Normal);
    }
    fn ime_cursor_area(&mut self, position: [f64; 2], size: [f64; 2]) {
        self.0.set_ime_cursor_area(
            LogicalPosition::new(position[0], position[1]),
            LogicalSize::new(size[0], size[1]),
        );
    }
    fn read_clipboard(&mut self) -> Result<String, String> {
        #[cfg(windows)]
        {
            clipboard_win::get_clipboard_string().map_err(|e| format!("Clipboard read failed: {e}"))
        }
        #[cfg(not(windows))]
        {
            Err("Clipboard unavailable on this platform".into())
        }
    }
    fn write_clipboard(&mut self, text: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            clipboard_win::set_clipboard_string(text)
                .map_err(|e| format!("Clipboard write failed: {e}"))
        }
        #[cfg(not(windows))]
        {
            let _ = text;
            Err("Clipboard unavailable on this platform".into())
        }
    }
}
