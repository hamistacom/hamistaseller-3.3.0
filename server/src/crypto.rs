//! Passwords (Argon2), the server's Ed25519 signing key, short-lived licence tokens, and random tokens.
//!
//! Licence token `HC1.<payload>.<sig>`: payload is base64url(JSON) of the claims, sig is base64url of
//! the Ed25519 signature over the payload bytes. The desktop app embeds the public key and verifies
//! the token offline between heartbeats — so the server sets the valid time, not the Windows clock,
//! and revoking a user on the server takes effect at the next heartbeat (token lifetime, e.g. 72h).
use crate::error::{bad, unauthorized, AppResult};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn hash_password(pw: &str) -> AppResult<String> {
    let mut raw = [0u8; 16];
    getrandom::getrandom(&mut raw).map_err(|e| bad(format!("rng: {e}")))?;
    let salt = SaltString::encode_b64(&raw).map_err(|e| bad(format!("salt: {e}")))?;
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| bad(format!("hash: {e}")))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok()).unwrap_or(false)
}

/// Verify against a throwaway hash so a missing username costs the same time as a real one
/// (defeats username enumeration by timing). The result is ignored.
pub fn verify_dummy(pw: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let h = DUMMY.get_or_init(|| hash_password("timing-equalizer").unwrap_or_default());
    let _ = verify_password(pw, h);
}

/// 32-byte url-safe random token (admin sessions, etc.).
pub fn random_token() -> String {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("os rng");
    B64.encode(b)
}

/// The signing key, loaded from its hex seed or generated on first run.
#[derive(Clone)]
pub struct Signer0 {
    key: SigningKey,
}

impl Signer0 {
    pub fn from_seed_hex(hex: &str) -> AppResult<Self> {
        let seed = hex32(hex).ok_or_else(|| bad("کلید امضا نامعتبر است."))?;
        Ok(Self { key: SigningKey::from_bytes(&seed) })
    }
    pub fn generate() -> (Self, String) {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("os rng");
        (Self { key: SigningKey::from_bytes(&seed) }, hex(&seed))
    }
    pub fn public_hex(&self) -> String {
        hex(&self.key.verifying_key().to_bytes())
    }
    pub fn sign_token(&self, claims: &Claims) -> AppResult<String> {
        let payload = serde_json::to_vec(claims).map_err(|e| bad(e.to_string()))?;
        let sig = self.key.sign(&payload);
        Ok(format!("HC1.{}.{}", B64.encode(&payload), B64.encode(sig.to_bytes())))
    }
}

/// What the desktop app reads out of a licence token. Times are unix seconds (UTC).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Claims {
    pub user: String,       // username
    pub customer: String,   // display name
    pub machine: String,    // machine code this token is bound to
    pub plan: String,
    pub iat: i64,
    pub exp: i64,           // short: heartbeat window (offline grace)
    pub sub_expires: String, // subscription end 'YYYY-MM-DD' (shown to the user)
    pub sources: Vec<String>, // allowed extraction sources ([] = all)
    pub megabank: bool,
    pub scan_left: i64,     // -1 = unlimited
    pub send_left: i64,
}

/// Verify a token with a public key (mirrors the desktop side; used by tests).
pub fn verify_token(public_hex: &str, token: &str) -> AppResult<Claims> {
    let key = VerifyingKey::from_bytes(&hex32(public_hex).ok_or_else(|| unauthorized("کلید عمومی نامعتبر"))?)
        .map_err(|_| unauthorized("کلید عمومی نامعتبر"))?;
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() != 3 || parts[0] != "HC1" {
        return Err(unauthorized("توکن نامعتبر"));
    }
    let payload = B64.decode(parts[1]).map_err(|_| unauthorized("توکن نامعتبر"))?;
    let sig = B64.decode(parts[2]).map_err(|_| unauthorized("توکن نامعتبر"))?;
    let sig = Signature::from_slice(&sig).map_err(|_| unauthorized("توکن نامعتبر"))?;
    key.verify(&payload, &sig).map_err(|_| unauthorized("امضای توکن نامعتبر"))?;
    serde_json::from_slice(&payload).map_err(|_| unauthorized("توکن نامعتبر"))
}

/// Stable per-PC code from a raw machine id (same shape the desktop sends: XXXX-XXXX-XXXX-XXXX).
pub fn machine_code(raw: &str) -> String {
    let digest = Sha256::digest(format!("hamista-cloud|{}", raw.trim().to_lowercase()).as_bytes());
    let h: String = digest.iter().take(8).map(|b| format!("{b:02X}")).collect();
    h.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect::<Option<_>>()?;
    v.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let h = hash_password("S3cret!").unwrap();
        assert!(verify_password("S3cret!", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn token_roundtrip_and_tamper() {
        let (signer, _seed) = Signer0::generate();
        let pk = signer.public_hex();
        let claims = Claims {
            user: "ali".into(),
            customer: "کافه".into(),
            machine: "3F25-04E0-4F89-11D3".into(),
            plan: "سالانه".into(),
            iat: 1000,
            exp: 2000,
            sub_expires: "2027-01-01".into(),
            sources: vec!["maps".into()],
            megabank: true,
            scan_left: 500,
            send_left: -1,
        };
        let tok = signer.sign_token(&claims).unwrap();
        let got = verify_token(&pk, &tok).unwrap();
        assert_eq!(got.user, "ali");
        assert_eq!(got.scan_left, 500);
        // any edit breaks the signature
        let mut parts: Vec<&str> = tok.splitn(3, '.').collect();
        let forged = format!("HC1.{}.{}", B64.encode(br#"{"user":"ali","scan_left":999999}"#), parts.pop().unwrap());
        assert!(verify_token(&pk, &forged).is_err());
        // wrong key
        let (other, _) = Signer0::generate();
        assert!(verify_token(&other.public_hex(), &tok).is_err());
    }

    #[test]
    fn machine_code_shape() {
        let m = machine_code("abc-123");
        assert_eq!(m.len(), 19);
        assert_eq!(m.matches('-').count(), 3);
        assert_eq!(machine_code("abc-123"), m); // stable
    }
}
