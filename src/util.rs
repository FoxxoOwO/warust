//! Utility functions

use chrono::{DateTime, Utc};
use whatsapp_rust::wacore_binary::{Jid, JidExt};

/// Check if a display name is an actual human name (not a raw JID or phone number)
pub fn is_real_name(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() 
        || trimmed.ends_with("@lid") 
        || trimmed.ends_with("@s.whatsapp.net") 
        || trimmed.starts_with("User:") 
        || trimmed.starts_with("Group:")
        || trimmed.starts_with("Broadcast:")
    {
        return false;
    }
    let non_plus = trimmed.strip_prefix('+').unwrap_or(trimmed);
    non_plus.chars().any(|c| c.is_alphabetic())
}

/// Shorten a JID user part to at most `n` chars without splitting UTF-8
fn short_user(user: &str, n: usize) -> String {
    user.chars().take(n).collect()
}

/// Format JID for display (phone number formatting)
pub fn format_jid(jid: &Jid) -> String {
    let user = jid.user();
    let server = jid.server();
    
    match server.as_str() {
        "s.whatsapp.net" => {
            // Try to format as phone number
            if user.len() >= 10 && user.chars().all(|c| c.is_ascii_digit()) {
                format_phone(user)
            } else {
                user.to_string()
            }
        }
        "lid" => format!("User:{}", short_user(user, 8)),
        "g.us" => format!("Group:{}", short_user(user, 8)),
        "broadcast" => format!("Broadcast:{}", short_user(user, 8)),
        "newsletter" => format!("Channel:{}", user),
        _ => jid.to_string(),
    }
}

/// Simple phone number formatting
fn format_phone(digits: &str) -> String {
    if (digits.starts_with("420") || digits.starts_with("421")) && digits.len() == 12 {
        // Czech / Slovak: +420 XXX XXX XXX
        format!("+{} {} {} {}", &digits[0..3], &digits[3..6], &digits[6..9], &digits[9..12])
    } else if digits.len() == 11 && digits.starts_with('1') {
        // US/Canada format: +1 (XXX) XXX-XXXX
        format!("+{} ({}) {}-{}", &digits[0..1], &digits[1..4], &digits[4..7], &digits[7..11])
    } else if digits.len() == 12 && digits.starts_with("44") {
        // UK: +44 XXXX XXXXXX
        format!("+44 {} {}", &digits[2..6], &digits[6..12])
    } else if digits.len() >= 10 {
        // Generic: split into country and rest grouped by 3
        let (cc, rest) = if digits.len() == 12 {
            (&digits[..3], &digits[3..])
        } else if digits.len() == 11 {
            (&digits[..2], &digits[2..])
        } else {
            (&digits[..digits.len()-9], &digits[digits.len()-9..])
        };
        format!("+{} {}", cc, rest)
    } else {
        digits.to_string()
    }
}

/// Format timestamp for display
pub fn format_timestamp(ts: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    
    if diff.num_days() == 0 {
        // Today - show time
        ts.format("%H:%M").to_string()
    } else if diff.num_days() == 1 {
        "Yesterday".to_string()
    } else if diff.num_days() < 7 {
        // This week - show day name
        ts.format("%a").to_string()
    } else {
        // Older - show date
        ts.format("%d.%m.%y").to_string()
    }
}

/// Format timestamp for message bubbles (more precise)
pub fn format_message_time(ts: DateTime<Utc>) -> String {
    ts.format("%H:%M").to_string()
}

/// Truncate text for preview (char-safe, never splits UTF-8 sequences)
pub fn truncate(text: &str, max_len: usize) -> String {
    let count = text.chars().count();
    if count <= max_len {
        return text.to_string();
    }
    let keep = max_len.saturating_sub(3);
    let mut out: String = text.chars().take(keep).collect();
    out.push_str("...");
    out
}

/// Get initials from name for avatar
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// Parse JID from string with fallback (handles raw phone numbers as well)
pub fn parse_jid(s: &str) -> Option<Jid> {
    let clean = s.trim();
    if clean.is_empty() {
        return None;
    }
    if clean.contains('@') {
        clean.parse().ok()
    } else {
        // Strip '+' and whitespace/dashes
        let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            Some(Jid::pn(digits))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_phone_us() {
        assert_eq!(format_phone("15551234567"), "+1 (555) 123-4567");
    }

    #[test]
    fn test_format_phone_cz() {
        assert_eq!(format_phone("420723755007"), "+420 723 755 007");
        assert_eq!(format_phone("421902685099"), "+421 902 685 099");
    }
    
    #[test]
    fn test_initials() {
        assert_eq!(initials("John Doe"), "JD");
        assert_eq!(initials("Alice"), "A");
        assert_eq!(initials("  Bob  Smith  "), "BS");
    }

    #[test]
    fn test_is_real_name() {
        assert!(is_real_name("Teresa Foltová"));
        assert!(is_real_name("Martin"));
        assert!(is_real_name("Mamka 💕"));
        assert!(!is_real_name("+420 737 526 533"));
        assert!(!is_real_name("420737526533"));
        assert!(!is_real_name("272825097166848@lid"));
        assert!(!is_real_name("User:27282509"));
        assert!(!is_real_name(""));
    }

    #[test]
    fn test_truncate_is_utf8_safe() {
        // Byte slicing on these used to panic.
        let czech = "Příliš žluťoučký kůň úpěl ďábelské ódy";
        let emoji = "😀😀😀😀😀😀😀😀😀😀";
        for len in 0..40 {
            let _ = truncate(czech, len);
            let _ = truncate(emoji, len);
        }
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate(czech, 5).chars().count(), 5);
        assert!(truncate(czech, 5).ends_with("..."));
    }

    #[test]
    fn test_format_jid_short_server_names() {
        // Short LID users used to hit an out-of-bounds slice.
        let jid: Jid = "123@lid".parse().unwrap();
        assert!(format_jid(&jid).starts_with("User:"));
    }
}