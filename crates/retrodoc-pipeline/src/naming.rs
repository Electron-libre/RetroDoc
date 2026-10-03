//! One way to compare names across passes: file names, identifiers and
//! entity names written in different cases or number must meet.

/// `ContractSigner`, `contract_signer`, `contract-signers` → `contractsigner`:
/// lowercase, no separator, no plural `s` (kept on very short names, where it
/// is more often part of the word: `bus`).
pub(crate) fn normalize(name: &str) -> String {
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
