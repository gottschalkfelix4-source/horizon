//! Text search in the terminal. The terminal's view is an ordinary chest screen the server opened, so
//! the client half of this mod listens for typing while that screen is open (menu `generic_9x6`,
//! titled `block.ae2.terminal`): letters build the search text, Backspace removes one, and the text is
//! sent to the server (`ae2:search`) which filters the listing. Typing never reaches the game's own
//! keys while the terminal is open (E would close the screen); Escape still closes it.
use std::sync::{Mutex, OnceLock};

use mod_api::{
    ClientHost, ClientTickPostEvent, Flow, PayloadId, ScreenCharTypedPreEvent, ScreenClosingEvent,
    ScreenKeyPressedPreEvent, ScreenView, ToServer, text::TextContent,
};

/// What the player has typed, and whether it still has to be sent.
struct Typed {
    text: String,
    dirty: bool,
}

static TYPED: Mutex<Typed> = Mutex::new(Typed { text: String::new(), dirty: false });
pub static SEARCH: OnceLock<ToServer<String>> = OnceLock::new();

pub const PAYLOAD: &str = "ae2:search";

fn typed() -> std::sync::MutexGuard<'static, Typed> {
    TYPED.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether `screen` is the terminal's view.
pub fn is_terminal(screen: &ScreenView<'_>) -> bool {
    screen.menu == Some("minecraft:generic_9x6")
        && screen.title.is_some_and(|t| {
            matches!(&t.content, TextContent::Translatable { key, .. } if key == "block.ae2.terminal")
        })
}

/// The character a key name types, for the keys that would otherwise act on the screen.
fn typing_key(name: &str) -> bool {
    match name.strip_prefix("key.keyboard.") {
        Some(rest) => rest.chars().count() == 1 || rest == "space" || rest == "minus" || rest == "period",
        None => false,
    }
}

pub fn key_pressed(event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
    if !is_terminal(&event.screen) {
        return Flow::Continue;
    }
    if event.key == "key.keyboard.backspace" {
        let mut t = typed();
        if t.text.pop().is_some() {
            t.dirty = true;
        }
        return Flow::Cancel;
    }
    // letters and digits are text here, not the inventory key or the hotbar keys
    if typing_key(&event.key) { Flow::Cancel } else { Flow::Continue }
}

pub fn char_typed(event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
    if !is_terminal(&event.screen) {
        return Flow::Continue;
    }
    if !event.c.is_control() && typed().text.chars().count() < 32 {
        let mut t = typed();
        t.text.push(event.c);
        t.dirty = true;
    }
    Flow::Cancel
}

/// Sends changed text to the server.
pub fn send(host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
    let text = {
        let mut t = typed();
        if !t.dirty {
            return;
        }
        t.dirty = false;
        t.text.clone()
    };
    if let Some(token) = SEARCH.get() {
        let _ = token.send(host, &text);
    }
}

/// Forgets the text when the screen closes (a terminal opens with no search).
pub fn closing(_: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
    let mut t = typed();
    t.text.clear();
    t.dirty = false;
}

/// The id of the payload.
pub fn id() -> PayloadId {
    PayloadId(PAYLOAD.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_space_are_typing_but_function_keys_are_not() {
        assert!(typing_key("key.keyboard.e"));
        assert!(typing_key("key.keyboard.3"));
        assert!(typing_key("key.keyboard.space"));
        assert!(!typing_key("key.keyboard.escape"));
        assert!(!typing_key("key.keyboard.enter"));
        assert!(!typing_key("key.mouse.left"));
    }
}
