//! The on-screen keyboard, which the Network screen raises for a passphrase and the Labels
//! and Settings > Gameplay screens for an account: a modal over the backdrop, titled with what
//! it asks for.

use omega_ui::{Draw, KeyboardFaces, KeyboardScreen};

use crate::app::{App, Phase};
use crate::list_view::TypingView;
use crate::wifi::shown;

impl App {
    /// The keyboard in use, and what it is asking for.
    pub fn typing(&self) -> Option<TypingView<'_>> {
        typing_of(&self.phase).or_else(|| self.link_typing_view())
    }

    /// The keyboard's faces as the sync last baked them. They stay once the keyboard is put
    /// away, so it can still be drawn sliding out.
    pub fn set_keyboard_faces(&mut self, faces: KeyboardFaces) {
        self.keyboard_faces = Some(faces);
    }

    pub fn keyboard_faces(&self) -> Option<&KeyboardFaces> {
        self.keyboard_faces.as_ref()
    }

    pub(crate) fn draw_keyboard(&self, t: &TypingView, out: &mut Vec<Draw>) {
        KeyboardScreen {
            keys: t.keys,
            faces: self.keyboard_faces.as_ref(),
        }
        .draw(out);
    }
}

/// The keyboard `phase` has up, and what it is asking for.
pub(crate) fn typing_of(phase: &Phase) -> Option<TypingView<'_>> {
    match phase {
        Phase::Network(screen) => screen.typing.as_ref().map(|t| TypingView {
            prompt: if t.refused {
                "A passphrase has 8 to 63 characters".to_string()
            } else {
                format!("Passphrase for {}", shown(&t.ssid))
            },
            keys: &t.keys,
            refused: t.refused,
        }),
        Phase::Labels(screen) => screen.typing.as_ref().map(|t| TypingView {
            prompt: t.prompt(),
            keys: &t.keys,
            refused: false,
        }),
        Phase::Options(screen) => screen.typing.as_ref().map(|t| TypingView {
            prompt: t.prompt(),
            keys: &t.keys,
            refused: false,
        }),
        _ => None,
    }
}
