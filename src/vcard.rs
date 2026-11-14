//! Minimal vCard 3.0 writer — the format WhatsApp puts in `ContactMessage`.

/// Escapes a value for a vCard 3.0 text field.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Phone digits of a JID: `420123456789:12@s.whatsapp.net` → `420123456789`.
pub fn jid_digits(jid: &str) -> String {
    jid.split('@')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect()
}

/// A vCard WhatsApp and other clients understand for `phone`.
///
/// `waid` is what links the card back to a WhatsApp account; the human value is
/// the international number.
pub fn build_vcard(display_name: &str, phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    format!(
        "BEGIN:VCARD\nVERSION:3.0\nFN:{name}\nTEL;type=CELL;type=VOICE;waid={digits}:+{digits}\nEND:VCARD",
        name = escape(display_name),
        digits = digits
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_carries_name_waid_and_international_number() {
        let card = build_vcard("Petr Novák", "420602123456");
        assert!(card.starts_with("BEGIN:VCARD\nVERSION:3.0\n"));
        assert!(card.contains("FN:Petr Novák\n"));
        assert!(card.contains("waid=420602123456:+420602123456"));
        assert!(card.ends_with("END:VCARD"));
    }

    #[test]
    fn separators_in_a_name_are_escaped() {
        // Semicolon and comma are field separators in vCard 3.0.
        assert_eq!(escape("Novák; Petr, ml."), "Novák\\; Petr\\, ml.");
        assert_eq!(escape("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn jid_digits_drop_server_and_device() {
        assert_eq!(jid_digits("420602123456:12@s.whatsapp.net"), "420602123456");
        assert_eq!(jid_digits("420602123456@s.whatsapp.net"), "420602123456");
        assert_eq!(jid_digits("123@lid"), "123");
        assert_eq!(jid_digits(""), "");
    }
}
