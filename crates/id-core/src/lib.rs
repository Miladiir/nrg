//! Core ID generation and validation logic for German energy market identifiers.
//! This crate compiles to both native Rust (for the backend) and WebAssembly (for the frontend).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Random number generation (platform-specific)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn random_digit() -> u8 {
    use rand::Rng;
    rand::thread_rng().gen_range(0..10)
}

#[cfg(not(target_arch = "wasm32"))]
fn random_nonzero_digit() -> u8 {
    use rand::Rng;
    rand::thread_rng().gen_range(1..10)
}

#[cfg(not(target_arch = "wasm32"))]
fn random_alphanum_upper() -> char {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let idx = rand::thread_rng().gen_range(0..CHARS.len());
    CHARS[idx] as char
}

#[cfg(target_arch = "wasm32")]
fn js_random_u32() -> u32 {
    use js_sys::Math;
    (Math::random() * (u32::MAX as f64)) as u32
}

#[cfg(target_arch = "wasm32")]
fn random_digit() -> u8 {
    (js_random_u32() % 10) as u8
}

#[cfg(target_arch = "wasm32")]
fn random_nonzero_digit() -> u8 {
    (js_random_u32() % 9 + 1) as u8
}

#[cfg(target_arch = "wasm32")]
fn random_alphanum_upper() -> char {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let idx = (js_random_u32() as usize) % CHARS.len();
    CHARS[idx] as char
}

// ─────────────────────────────────────────────────────────────────────────────
// MaLo-ID  (Marktlokations-ID)
// ─────────────────────────────────────────────────────────────────────────────
// Format: 11 purely numeric digits
//   - Position 1: 1–9 (1–3 = DVGW, 4–9 = BDEW)
//   - Positions 2–10: digits 0–9
//   - Position 11: check digit (Lok-Waggon algorithm)
//
// Lok-Waggon:
//   odd_sum  = sum of digits at 1-based odd positions (1,3,5,7,9)
//   even_sum = sum of digits at 1-based even positions (2,4,6,8,10)
//   total    = odd_sum + even_sum * 2
//   check    = (10 - (total % 10)) % 10

pub fn calculate_malo_checksum(base10: &str) -> Result<u8, String> {
    if base10.len() != 10 {
        return Err(format!("Expected 10 digits, got {}", base10.len()));
    }
    let mut odd_sum: i32 = 0;
    let mut even_sum: i32 = 0;
    for (i, c) in base10.chars().enumerate() {
        let d = c.to_digit(10).ok_or_else(|| format!("Non-digit char '{}' at position {}", c, i + 1))? as i32;
        if (i + 1) % 2 == 1 {
            odd_sum += d;
        } else {
            even_sum += d;
        }
    }
    let total = odd_sum + even_sum * 2;
    Ok(((10 - (total % 10)) % 10) as u8)
}

pub struct MaloInfo {
    pub id: String,
    pub checksum: u8,
    pub issuer: &'static str,
}

pub fn validate_malo(id: &str) -> Result<MaloInfo, String> {
    if id.len() != 11 {
        return Err(format!("MaLo-ID must be 11 digits, got {}", id.len()));
    }
    let first = id.chars().next().unwrap().to_digit(10)
        .ok_or("First character must be a digit")?;
    if first < 1 {
        return Err("First digit must be 1–9".to_string());
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("MaLo-ID must be purely numeric".to_string());
    }
    let base = &id[..10];
    let expected = calculate_malo_checksum(base)?;
    let actual = id.chars().last().unwrap().to_digit(10).unwrap() as u8;
    if actual != expected {
        return Err(format!("Invalid checksum: expected {}, got {}", expected, actual));
    }
    let issuer = if first <= 3 { "DVGW" } else { "BDEW" };
    Ok(MaloInfo { id: id.to_string(), checksum: actual, issuer })
}

pub fn generate_malo() -> String {
    loop {
        let first = random_nonzero_digit();
        let mut base = String::with_capacity(10);
        base.push(char::from_digit(first as u32, 10).unwrap());
        for _ in 0..9 {
            base.push(char::from_digit(random_digit() as u32, 10).unwrap());
        }
        if let Ok(check) = calculate_malo_checksum(&base) {
            return format!("{}{}", base, check);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MeLo-ID  (Messlokations-ID / Zählpunktbezeichnung)
// ─────────────────────────────────────────────────────────────────────────────
// Format: 33 characters, no separators
//   Segment 1 – Country code (ISO 3166): 2 chars, e.g. "DE"
//   Segment 2 – Network operator:        6 digits
//   Segment 3 – Postal code:             5 digits
//   Segment 4 – Meter point number:     20 uppercase alphanumeric (A–Z, 0–9)
//
// Example: DE00056266802AO6G56M11SN51G21M24S

pub fn validate_melo(id: &str) -> Result<(), String> {
    if id.len() != 33 {
        return Err(format!("MeLo-ID must be 33 characters, got {}", id.len()));
    }
    if !id.starts_with("DE") {
        return Err("MeLo-ID country code must be 'DE'".to_string());
    }
    let network_op = &id[2..8];
    let postal = &id[8..13];
    let meter = &id[13..33];
    if !network_op.chars().all(|c| c.is_ascii_digit()) {
        return Err("Network operator segment (positions 3–8) must be 6 digits".to_string());
    }
    if !postal.chars().all(|c| c.is_ascii_digit()) {
        return Err("Postal code segment (positions 9–13) must be 5 digits".to_string());
    }
    if !meter.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err("Meter point segment (positions 14–33) must be 20 uppercase alphanumeric characters".to_string());
    }
    Ok(())
}

pub fn generate_melo() -> String {
    let network_op: String = (0..6).map(|_| char::from_digit(random_digit() as u32, 10).unwrap()).collect();
    let postal: String = (0..5).map(|_| char::from_digit(random_digit() as u32, 10).unwrap()).collect();
    let meter: String = (0..20).map(|_| random_alphanum_upper()).collect();
    format!("DE{}{}{}", network_op, postal, meter)
}

// ─────────────────────────────────────────────────────────────────────────────
// NeLo-ID  (Netzlokations-ID)
// ─────────────────────────────────────────────────────────────────────────────
// Format: 11 characters
//   - Position 1: always 'E'
//   - Positions 2–10: uppercase letter A–Z or digit 0–9
//   - Position 11: numeric check digit (ASCII-Verfahren)
//
// ASCII-Verfahren (same weighted formula as Lok-Waggon but character values differ):
//   char_value = digit value (0–9) for digits, ASCII code for letters (A=65…Z=90)
//   odd_sum  = sum of char_values at 1-based odd positions (1,3,5,7,9)
//   even_sum = sum of char_values at 1-based even positions (2,4,6,8,10)
//   total    = odd_sum + even_sum * 2
//   check    = (10 - (total % 10)) % 10

fn char_value(c: char) -> i32 {
    if c.is_ascii_digit() {
        c.to_digit(10).unwrap() as i32
    } else {
        c as i32
    }
}

pub fn calculate_nelo_checksum(base10: &str) -> Result<u8, String> {
    if base10.len() != 10 {
        return Err(format!("Expected 10 characters, got {}", base10.len()));
    }
    let first = base10.chars().next().unwrap();
    if first != 'E' {
        return Err("NeLo-ID must start with 'E'".to_string());
    }
    let valid_chars: bool = base10.chars().skip(1).all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if !valid_chars {
        return Err("NeLo-ID positions 2–10 must be uppercase letters or digits".to_string());
    }
    let mut odd_sum: i32 = 0;
    let mut even_sum: i32 = 0;
    for (i, c) in base10.chars().enumerate() {
        let v = char_value(c);
        if (i + 1) % 2 == 1 {
            odd_sum += v;
        } else {
            even_sum += v;
        }
    }
    let total = odd_sum + even_sum * 2;
    Ok(((10 - (total % 10)) % 10) as u8)
}

pub fn validate_nelo(id: &str) -> Result<(), String> {
    if id.len() != 11 {
        return Err(format!("NeLo-ID must be 11 characters, got {}", id.len()));
    }
    if !id.starts_with('E') {
        return Err("NeLo-ID must start with 'E'".to_string());
    }
    let last = id.chars().last().unwrap();
    if !last.is_ascii_digit() {
        return Err("NeLo-ID check digit (position 11) must be numeric".to_string());
    }
    let base = &id[..10];
    let expected = calculate_nelo_checksum(base)?;
    let actual = last.to_digit(10).unwrap() as u8;
    if actual != expected {
        return Err(format!("Invalid checksum: expected {}, got {}", expected, actual));
    }
    Ok(())
}

pub fn generate_nelo() -> String {
    loop {
        let mut base = String::with_capacity(10);
        base.push('E');
        for _ in 0..9 {
            base.push(random_alphanum_upper());
        }
        if let Ok(check) = calculate_nelo_checksum(&base) {
            return format!("{}{}", base, check);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WebAssembly exports
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_generate_malo() -> String {
    generate_malo()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_validate_malo(id: &str) -> String {
    match validate_malo(id) {
        Ok(info) => format!(
            r#"{{"valid":true,"checksum":{},"issuer":"{}","id":"{}"}}"#,
            info.checksum, info.issuer, info.id
        ),
        Err(e) => format!(r#"{{"valid":false,"error":{}}}"#, json_string(&e)),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_generate_melo() -> String {
    generate_melo()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_validate_melo(id: &str) -> String {
    match validate_melo(id) {
        Ok(()) => format!(r#"{{"valid":true,"id":{}}}"#, json_string(id)),
        Err(e) => format!(r#"{{"valid":false,"error":{}}}"#, json_string(&e)),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_generate_nelo() -> String {
    generate_nelo()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_validate_nelo(id: &str) -> String {
    match validate_nelo(id) {
        Ok(()) => format!(r#"{{"valid":true,"id":{}}}"#, json_string(id)),
        Err(e) => format!(r#"{{"valid":false,"error":{}}}"#, json_string(&e)),
    }
}

#[cfg(target_arch = "wasm32")]
fn json_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malo_checksum_example() {
        assert_eq!(calculate_malo_checksum("4137355924").unwrap(), 1);
    }

    #[test]
    fn malo_validate_valid() {
        assert!(validate_malo("41373559241").is_ok());
    }

    #[test]
    fn malo_validate_wrong_check() {
        assert!(validate_malo("41373559240").is_err());
    }

    #[test]
    fn malo_generate_is_valid() {
        for _ in 0..20 {
            let id = generate_malo();
            assert!(validate_malo(&id).is_ok(), "Generated MaLo-ID {} is invalid", id);
        }
    }

    #[test]
    fn melo_validate_valid() {
        // Correct DE format: 2 + 6 + 5 + 20 = 33 chars
        assert!(validate_melo("DE00056266802AO6G56M11SN51G21M24S").is_ok());
        // Wrong length
        assert!(validate_melo("DE00056266802AO6G56M11SN51G21M24").is_err()); // 32 chars
        assert!(validate_melo("DE00056266802AO6G56M11SN51G21M24SS").is_err()); // 34 chars
        // Wrong country code
        assert!(validate_melo("GB00056266802AO6G56M11SN51G21M24S").is_err());
        // Lowercase in meter segment
        assert!(validate_melo("DE00056266802ao6g56m11sn51g21m24s").is_err());
    }

    #[test]
    fn melo_generate_is_valid() {
        for _ in 0..20 {
            let id = generate_melo();
            assert!(validate_melo(&id).is_ok(), "Generated MeLo-ID {} is invalid", id);
        }
    }

    #[test]
    fn nelo_checksum_example() {
        // "E113735592": E=69(odd) 1(even) 1(odd) 3(even) 7(odd) 3(even) 5(odd) 5(even) 9(odd) 2(even)
        // odd_sum  = 69+1+7+5+9 = 91
        // even_sum = (1+3+3+5+2)*2 = 28
        // total = 119  →  check = (10 - 119%10) % 10 = 1
        assert_eq!(calculate_nelo_checksum("E113735592").unwrap(), 1);
    }

    #[test]
    fn nelo_checksum_alpha() {
        // "EABC123DEF": E=69(odd) A=65(even) B=66(odd) C=67(even) 1(odd) 2(even) 3(odd) D=68(even) E=69(odd) F=70(even)
        // odd_sum  = 69+66+1+3+69 = 208
        // even_sum = (65+67+2+68+70)*2 = 544
        // total = 752  →  check = (10 - 752%10) % 10 = 8
        assert_eq!(calculate_nelo_checksum("EABC123DEF").unwrap(), 8);
    }

    #[test]
    fn nelo_validate_alpha() {
        assert!(validate_nelo("EABC123DEF8").is_ok());
        assert!(validate_nelo("EABC123DEF0").is_err());
    }

    #[test]
    fn nelo_generate_is_valid() {
        for _ in 0..20 {
            let id = generate_nelo();
            assert!(validate_nelo(&id).is_ok(), "Generated NeLo-ID {} is invalid", id);
        }
    }
}
