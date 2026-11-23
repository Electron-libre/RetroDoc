//! Text helpers shared by the crates that write a prompt, a page or an answer.

use std::time::Duration;

/// `12400` → `12,400`.
#[must_use]
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `7s`, `3m12s`, `1h05m`.
#[must_use]
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs % 3600 / 60),
    }
}

/// `text` on one line, so that it can't forge a line of the prompt or of the list it is written in.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The ASCII letters of a lowercase accented Latin letter, if it is one.
fn latin_base(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
        'ł' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'ř' => "r",
        'ś' | 'š' | 'ş' | 'ș' => "s",
        'ť' | 'ţ' | 'ț' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'ß' => "ss",
        _ => return None,
    })
}

/// `text` lowercased with its accented Latin letters, composed or decomposed, folded to their base
/// letters, so that `Français` and `francais` compare equal; any other letter is kept.
#[must_use]
pub fn fold_latin(text: &str) -> String {
    let mut folded = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if ('\u{300}'..='\u{36f}').contains(&c) {
            // A combining accent (a decomposed letter): its base letter is already in.
        } else if let Some(base) = latin_base(c) {
            folded.push_str(base);
        } else {
            folded.push(c);
        }
    }
    folded
}

#[must_use]
/// `ContractSigner`, `contract_signer`, `contract-signers` → `contractsigner`:
/// lowercase, no separator, no plural `s` (kept on very short names, where it
/// is more often part of the word: `bus`).
pub fn normalize(name: &str) -> String {
    let lower: String = name
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    match lower.strip_suffix('s') {
        Some(singular) if singular.len() > 3 => singular.to_string(),
        _ => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_203_400), "1,203,400");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_secs(7)), "7s");
        assert_eq!(format_duration(Duration::from_secs(192)), "3m12s");
        assert_eq!(format_duration(Duration::from_mins(65)), "1h05m");
    }

    #[test]
    fn a_text_over_several_lines_is_put_on_one_line() {
        assert_eq!(
            one_line("  Place\nan  order\r\n\t[T1] x "),
            "Place an order [T1] x"
        );
        assert_eq!(one_line("\n \n"), "");
    }

    #[test]
    fn accented_latin_letters_fold_to_their_base_letters_and_others_are_kept() {
        assert_eq!(fold_latin("Préférences"), "preferences");
        assert_eq!(fold_latin("Pre\u{301}fe\u{301}rences"), "preferences");
        assert_eq!(fold_latin("Straße Œuvre Æsir"), "strasse oeuvre aesir");
        assert_eq!(fold_latin("İstanbul"), "istanbul");
        assert_eq!(fold_latin("Доставка 配送"), "доставка 配送");
    }

    #[test]
    fn case_separators_and_plural_are_ignored() {
        let expected = normalize("ContractSigner");
        assert_eq!(expected, "contractsigner");
        assert_eq!(normalize("contract_signer"), expected);
        assert_eq!(normalize("contract-signers"), expected);
        assert_eq!(normalize("Contract Signers"), expected);
    }

    #[test]
    fn short_names_keep_their_s() {
        assert_eq!(normalize("bus"), "bus");
        assert_eq!(normalize("docs"), "docs");
    }
}
