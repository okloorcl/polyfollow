use anyhow::Result;

pub fn validate_address(address: &str) -> Result<()> {
    let value = address.trim();
    if value.len() != 42 || !value.starts_with("0x") {
        anyhow::bail!("expected 0x-prefixed 20-byte address");
    }
    if !value[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("address contains non-hex characters");
    }
    Ok(())
}

pub fn normalize_address(address: &str) -> Result<String> {
    validate_address(address)?;
    Ok(address.trim().to_ascii_lowercase())
}
