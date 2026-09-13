//! Shared mathematical-source admission. LaTeX is data, never executable TeX.

pub(crate) const MAX_LATEX_BYTES: usize = 16 * 1024;

pub(crate) fn validate_latex(source: &str) -> Result<(), String> {
    if source.trim().is_empty() || source.len() > MAX_LATEX_BYTES {
        return Err("A formula must contain between 1 and 16,384 UTF-8 bytes.".into());
    }
    if source
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
    {
        return Err("A formula contains invalid control characters.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_bytes_not_platform_specific_string_length() {
        assert!(validate_latex(&"α".repeat(MAX_LATEX_BYTES / 2)).is_ok());
        assert!(validate_latex(&"α".repeat(MAX_LATEX_BYTES / 2 + 1)).is_err());
        assert!(validate_latex("x\0y").is_err());
        assert!(validate_latex("   ").is_err());
        assert!(validate_latex("x\n+y").is_ok());
    }
}
