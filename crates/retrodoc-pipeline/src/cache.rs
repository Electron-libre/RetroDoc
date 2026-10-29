//! The content hash that keys what the passes save under `.retrodoc/cache/` (source hashes,
//! fingerprints): a file or input whose hash is unchanged since the last run is not asked again.

use sha2::{Digest, Sha256};

/// SHA-256 of `content`, in hexadecimal.
#[must_use]
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_changes_with_content() {
        assert_ne!(hash_content("a"), hash_content("b"));
        assert_eq!(hash_content("a"), hash_content("a"));
    }
}
