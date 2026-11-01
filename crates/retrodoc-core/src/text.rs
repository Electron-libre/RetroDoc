//! Text helpers shared by the crates that write a prompt, a page or an answer.

/// `text` on one line, so that it can't forge a line of the prompt or of the list it is written in.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
}
