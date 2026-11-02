//! Text helpers shared by the crates that write a prompt, a page or an answer.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
