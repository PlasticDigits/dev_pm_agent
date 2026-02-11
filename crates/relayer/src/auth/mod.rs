//! Authentication and authorization.

use anyhow::Result;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // device_id
    pub admin_id: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

/// Create JWT for device.
pub fn create_jwt(
    device_id: Uuid,
    admin_id: Uuid,
    role: &str,
    secret: &str,
    ttl_secs: u64,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let claims = Claims {
        sub: device_id.to_string(),
        admin_id: admin_id.to_string(),
        role: role.to_string(),
        exp: now + ttl_secs as i64,
        iat: now,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

/// Validate JWT and return (device_id, admin_id, role).
pub fn validate_jwt(token: &str, secret: &str) -> Result<Option<(Uuid, Uuid, String)>> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    );
    match data {
        Ok(data) => {
            let device_id = Uuid::parse_str(&data.claims.sub)?;
            let admin_id = Uuid::parse_str(&data.claims.admin_id)?;
            Ok(Some((device_id, admin_id, data.claims.role)))
        }
        Err(_) => Ok(None),
    }
}

/// Hash API key for storage.
pub fn hash_api_key(key: &str) -> Result<String> {
    Ok(bcrypt::hash(key, bcrypt::DEFAULT_COST)?)
}

/// Generate a random API key (hex).
pub fn generate_api_key() -> String {
    use std::fmt::Write;
    let bytes: [u8; 32] = rand::random();
    let mut s = String::with_capacity(64);
    for b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

/// Generate TOTP secret (base32 for authenticator apps).
pub fn generate_totp_secret() -> Result<String> {
    use base32::Alphabet;
    let bytes: Vec<u8> = (0..20).map(|_| rand::random::<u8>()).collect();
    Ok(base32::encode(Alphabet::RFC4648 { padding: false }, &bytes))
}

/// Verify TOTP code.
pub fn verify_totp(secret: &str, code: &str) -> bool {
    use totp_rs::{Algorithm, TOTP};
    let secret_decoded = base32::decode(base32::Alphabet::RFC4648 { padding: false }, secret);
    let bytes = match secret_decoded {
        Some(b) if b.len() >= 16 => b,
        _ => return false,
    };
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes);
    let totp = match totp {
        Ok(t) => t,
        Err(_) => return false,
    };
    totp.check_current(code).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn generate_jwt_secret() -> String {
        (0..32)
            .map(|_| rand::random::<u8>())
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    #[test]
    fn jwt_roundtrip_with_generated_secret() {
        let secret = generate_jwt_secret();
        let device_id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();
        let role = "controller";

        let token = create_jwt(device_id, admin_id, role, &secret, 3600).unwrap();
        let parsed = validate_jwt(&token, &secret).unwrap();
        assert!(parsed.is_some());
        let (d, a, r) = parsed.unwrap();
        assert_eq!(d, device_id);
        assert_eq!(a, admin_id);
        assert_eq!(r, role);
    }

    #[test]
    fn jwt_rejects_wrong_secret() {
        let secret = generate_jwt_secret();
        let wrong_secret = generate_jwt_secret();
        let device_id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();

        let token = create_jwt(device_id, admin_id, "controller", &secret, 3600).unwrap();
        let parsed = validate_jwt(&token, &wrong_secret).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn generate_api_key_format() {
        let key = generate_api_key();
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_api_key_and_verify() {
        let key = generate_api_key();
        let hash = hash_api_key(&key).unwrap();
        assert!(bcrypt::verify(&key, &hash).unwrap());
    }

    #[test]
    fn totp_generate_and_verify() {
        use totp_rs::{Algorithm, TOTP};

        let secret = generate_totp_secret().unwrap();
        let decoded = base32::decode(base32::Alphabet::RFC4648 { padding: false }, &secret);
        let bytes = decoded.expect("decode secret");
        assert!(bytes.len() >= 16);

        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes).unwrap();
        let code = totp.generate_current().unwrap();

        assert!(verify_totp(&secret, &code));
    }

    #[test]
    fn totp_rejects_wrong_code() {
        let secret = generate_totp_secret().unwrap();
        assert!(!verify_totp(&secret, "000000"));
    }
}
