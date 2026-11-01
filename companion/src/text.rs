/// True when the text holds at least one Hangul syllable or jamo.
pub fn has_hangul(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3131}'..='\u{318E}' | '\u{AC00}'..='\u{D7A3}'))
}

/// Splits "TR#12 text" into (12, "text").
pub fn parse_tagged(text: &str) -> Option<(u64, &str)> {
    let rest = text.trim().strip_prefix("TR#")?;
    let (id, body) = rest.split_once(char::is_whitespace)?;
    Some((id.parse().ok()?, body.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hangul() {
        assert!(has_hangul("탱커 구함"));
        assert!(has_hangul("ㅋㅋ"));
        assert!(!has_hangul("lf tank"));
        assert!(!has_hangul("需要奶妈"));
    }

    #[test]
    fn parses_tag() {
        assert_eq!(parse_tagged("TR#12 need tank"), Some((12, "need tank")));
        assert_eq!(parse_tagged("need tank"), None);
    }
}
