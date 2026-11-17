//! The keyboard keys a mapping can take (the bindings themselves come from `ClientHost::key_bindings`).

/// The keyboard keys Controlling's "free keys" list walks: the ones a standard layout has.
pub fn keyboard_keys() -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    keys.extend(('a'..='z').map(|c| format!("key.keyboard.{c}")));
    keys.extend(('0'..='9').map(|c| format!("key.keyboard.{c}")));
    keys.extend((1..=12).map(|n| format!("key.keyboard.f{n}")));
    keys.extend((0..=9).map(|n| format!("key.keyboard.keypad.{n}")));
    for named in [
        "space",
        "tab",
        "caps.lock",
        "left.shift",
        "right.shift",
        "left.control",
        "right.control",
        "left.alt",
        "right.alt",
        "enter",
        "backspace",
        "insert",
        "delete",
        "home",
        "end",
        "page.up",
        "page.down",
        "up",
        "down",
        "left",
        "right",
        "grave.accent",
        "minus",
        "equal",
        "left.bracket",
        "right.bracket",
        "backslash",
        "semicolon",
        "apostrophe",
        "comma",
        "period",
        "slash",
        "keypad.add",
        "keypad.subtract",
        "keypad.multiply",
        "keypad.divide",
        "keypad.decimal",
        "keypad.enter",
    ] {
        keys.push(format!("key.keyboard.{named}"));
    }
    keys
}
