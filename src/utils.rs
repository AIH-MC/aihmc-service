use std::sync::LazyLock;

use regex::Regex;

pub fn offline_uuid(username: &str) -> String {
    let mut bytes = md5::compute(format!("OfflinePlayer:{username}").as_bytes()).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn parse_duration(value: &str) -> u64 {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)([dhms])").unwrap());
    PATTERN
        .captures_iter(&value.to_lowercase())
        .map(|capture| {
            let multiplier = match &capture[2] {
                "d" => 86400,
                "h" => 3600,
                "m" => 60,
                _ => 1,
            };
            capture[1]
                .parse::<u64>()
                .unwrap_or(0)
                .saturating_mul(multiplier)
        })
        .sum()
}

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password.as_bytes(), bcrypt::DEFAULT_COST)
}

pub fn check_password(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
    bcrypt::verify(password.as_bytes(), hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_offline_uuid_matches_python_reference() {
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
        assert_ne!(offline_uuid("Player"), offline_uuid("player"));
    }

    #[test]
    fn durations_match_reference() {
        assert_eq!(parse_duration("7d 10h 50m 20s"), 643820);
        assert_eq!(parse_duration("bad"), 0);
        assert_eq!(parse_duration("2D"), 172800);
    }

    #[test]
    fn passwords_support_unicode_and_legacy_truncation() {
        let hash = hash_password("密码").unwrap();
        assert!(check_password("密码", &hash).unwrap());
        assert!(!check_password("wrong", &hash).unwrap());
        let hash = hash_password(&format!("{}x", "a".repeat(72))).unwrap();
        assert!(check_password(&format!("{}y", "a".repeat(72)), &hash).unwrap());
    }
}
