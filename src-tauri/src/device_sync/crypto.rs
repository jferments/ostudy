use base64::{
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use ring::{
    agreement, digest, hkdf, hmac, rand,
    rand::SecureRandom,
    signature::{self, KeyPair},
};

use crate::{
    error::CommandError,
    platform::{SecretLocator, SecretStoreError, SecureSecretStore},
};

const IDENTITY_LOCATOR_KEY: &str = "identity/ed25519-pkcs8-v1";
const PAIRING_SECRET_BYTES: usize = 32;
const SESSION_KEY_BYTES: usize = 64;

pub(crate) struct IdentityKey {
    key_pair: signature::Ed25519KeyPair,
}

pub(crate) struct EphemeralKey {
    private: agreement::EphemeralPrivateKey,
    pub public_key: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct SessionKeys {
    pub send_key: [u8; 32],
    pub receive_key: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) enum SessionRole {
    Host,
    Joiner,
}

impl IdentityKey {
    pub(crate) fn load_or_create(
        secret_store: &dyn SecureSecretStore,
    ) -> Result<Self, CommandError> {
        let locator = identity_locator();
        if let Some(bytes) = secret_store.read(&locator).map_err(secret_store_error)? {
            let key_pair = signature::Ed25519KeyPair::from_pkcs8(bytes.as_bytes()).map_err(|_| {
                CommandError::new(
                    "device_sync_identity_invalid",
                    "The saved device identity key is invalid. Remove paired devices and reset synchronization identity.",
                )
            })?;
            return Ok(Self { key_pair });
        }

        let rng = rand::SystemRandom::new();
        let document = signature::Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| {
            CommandError::new(
                "device_sync_crypto_unavailable",
                "A secure device identity key could not be generated.",
            )
        })?;
        secret_store
            .write(&locator, document.as_ref())
            .map_err(secret_store_error)?;
        let key_pair = signature::Ed25519KeyPair::from_pkcs8(document.as_ref()).map_err(|_| {
            CommandError::new(
                "device_sync_crypto_unavailable",
                "The generated device identity key could not be opened.",
            )
        })?;
        Ok(Self { key_pair })
    }

    pub(crate) fn public_key(&self) -> &[u8] {
        self.key_pair.public_key().as_ref()
    }

    pub(crate) fn public_key_base64(&self) -> String {
        BASE64_STANDARD.encode(self.public_key())
    }

    pub(crate) fn fingerprint(&self) -> String {
        fingerprint(self.public_key())
    }

    pub(crate) fn sign(&self, message: &[u8]) -> String {
        BASE64_STANDARD.encode(self.key_pair.sign(message).as_ref())
    }
}

impl EphemeralKey {
    pub(crate) fn generate() -> Result<Self, CommandError> {
        let rng = rand::SystemRandom::new();
        let private = agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng).map_err(
            |_| {
                CommandError::new(
                    "device_sync_crypto_unavailable",
                    "An encrypted pairing key could not be generated.",
                )
            },
        )?;
        let public_key = private
            .compute_public_key()
            .map_err(|_| {
                CommandError::new(
                    "device_sync_crypto_unavailable",
                    "The encrypted pairing public key could not be generated.",
                )
            })?
            .as_ref()
            .to_vec();
        Ok(Self {
            private,
            public_key,
        })
    }

    pub(crate) fn derive(
        self,
        peer_public_key: &[u8],
        pairing_secret: &[u8],
        transcript: &[u8],
        role: SessionRole,
    ) -> Result<SessionKeys, CommandError> {
        if pairing_secret.len() != PAIRING_SECRET_BYTES {
            return Err(CommandError::new(
                "device_sync_invitation_invalid",
                "The pairing invitation contains an invalid secret.",
            ));
        }
        let peer = agreement::UnparsedPublicKey::new(&agreement::X25519, peer_public_key);
        agreement::agree_ephemeral(self.private, &peer, |shared| {
            derive_session_material(shared, pairing_secret, transcript, role)
        })
        .map_err(|_| {
            CommandError::new(
                "device_sync_handshake_failed",
                "The encrypted device pairing handshake failed.",
            )
        })?
    }
}

pub(crate) fn random_pairing_secret() -> Result<[u8; PAIRING_SECRET_BYTES], CommandError> {
    let mut value = [0_u8; PAIRING_SECRET_BYTES];
    rand::SystemRandom::new().fill(&mut value).map_err(|_| {
        CommandError::new(
            "device_sync_crypto_unavailable",
            "A secure pairing secret could not be generated.",
        )
    })?;
    Ok(value)
}

pub(crate) fn pairing_proof(secret: &[u8], transcript: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    BASE64_STANDARD.encode(hmac::sign(&key, transcript).as_ref())
}

pub(crate) fn verify_pairing_proof(
    secret: &[u8],
    transcript: &[u8],
    proof_base64: &str,
) -> Result<(), CommandError> {
    let proof = BASE64_STANDARD.decode(proof_base64).map_err(|_| {
        CommandError::new(
            "device_sync_handshake_failed",
            "The pairing proof is not valid base64.",
        )
    })?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    hmac::verify(&key, transcript, &proof).map_err(|_| {
        CommandError::new(
            "device_sync_handshake_failed",
            "The pairing proof did not match the scanned invitation.",
        )
    })
}

pub(crate) fn verify_signature(
    public_key_base64: &str,
    message: &[u8],
    signature_base64: &str,
) -> Result<(), CommandError> {
    let public_key = BASE64_STANDARD.decode(public_key_base64).map_err(|_| {
        CommandError::new(
            "device_sync_handshake_failed",
            "The paired device public key is invalid.",
        )
    })?;
    let signature = BASE64_STANDARD.decode(signature_base64).map_err(|_| {
        CommandError::new(
            "device_sync_handshake_failed",
            "The paired device signature is invalid.",
        )
    })?;
    signature::UnparsedPublicKey::new(&signature::ED25519, public_key)
        .verify(message, &signature)
        .map_err(|_| {
            CommandError::new(
                "device_sync_handshake_failed",
                "The paired device could not prove possession of its identity key.",
            )
        })
}

pub(crate) fn fingerprint(public_key: &[u8]) -> String {
    format!("{:x}", digest::digest(&digest::SHA256, public_key))
}

pub(crate) fn decode_standard(value: &str, label: &str) -> Result<Vec<u8>, CommandError> {
    BASE64_STANDARD.decode(value).map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            format!("The pairing invitation contains an invalid {label}."),
        )
    })
}

pub(crate) fn encode_url(value: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(value)
}

pub(crate) fn decode_url(value: &str) -> Result<Vec<u8>, CommandError> {
    URL_SAFE_NO_PAD.decode(value).map_err(|_| {
        CommandError::new(
            "device_sync_invitation_invalid",
            "The pairing invitation payload is not valid base64url.",
        )
    })
}

fn derive_session_material(
    shared_secret: &[u8],
    pairing_secret: &[u8],
    transcript: &[u8],
    role: SessionRole,
) -> Result<SessionKeys, CommandError> {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, pairing_secret);
    let pseudo_random_key = salt.extract(shared_secret);
    let info = [b"ostudy-device-sync-v1".as_slice(), transcript];
    let output = pseudo_random_key
        .expand(&info, OutputLength(SESSION_KEY_BYTES))
        .map_err(|_| crypto_error())?;
    let mut material = [0_u8; SESSION_KEY_BYTES];
    output.fill(&mut material).map_err(|_| crypto_error())?;
    let mut first = [0_u8; 32];
    let mut second = [0_u8; 32];
    first.copy_from_slice(&material[..32]);
    second.copy_from_slice(&material[32..]);
    material.fill(0);
    Ok(match role {
        SessionRole::Joiner => SessionKeys {
            send_key: first,
            receive_key: second,
        },
        SessionRole::Host => SessionKeys {
            send_key: second,
            receive_key: first,
        },
    })
}

#[derive(Clone, Copy)]
struct OutputLength(usize);

impl hkdf::KeyType for OutputLength {
    fn len(&self) -> usize {
        self.0
    }
}

fn identity_locator() -> SecretLocator {
    SecretLocator {
        namespace: "device_sync",
        key: IDENTITY_LOCATOR_KEY.to_string(),
    }
}

fn secret_store_error(error: SecretStoreError) -> CommandError {
    let message = match error {
        SecretStoreError::Unavailable => {
            "The operating-system secure store is unavailable for device pairing."
        }
        SecretStoreError::Locked => {
            "Unlock the operating-system secure store before pairing devices."
        }
        SecretStoreError::PermissionDenied => {
            "The operating system denied access to the device pairing identity."
        }
        SecretStoreError::Invalidated => {
            "The saved device pairing identity was invalidated by the operating system."
        }
        SecretStoreError::Failed => "The secure device pairing identity could not be accessed.",
    };
    CommandError::new("device_sync_secure_store_failed", message)
}

fn crypto_error() -> CommandError {
    CommandError::new(
        "device_sync_crypto_unavailable",
        "The encrypted device synchronization key could not be derived.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x25519_hkdf_derives_opposite_direction_keys() {
        let left = EphemeralKey::generate().expect("left key should generate");
        let left_public = left.public_key.clone();
        let right = EphemeralKey::generate().expect("right key should generate");
        let right_public = right.public_key.clone();
        let secret = random_pairing_secret().expect("secret should generate");
        let transcript = b"pairing transcript";
        let joiner = left
            .derive(&right_public, &secret, transcript, SessionRole::Joiner)
            .expect("joiner keys should derive");
        let host = right
            .derive(&left_public, &secret, transcript, SessionRole::Host)
            .expect("host keys should derive");
        assert_eq!(joiner.send_key, host.receive_key);
        assert_eq!(joiner.receive_key, host.send_key);
        assert_ne!(joiner.send_key, joiner.receive_key);
    }

    #[test]
    fn pairing_proof_rejects_changed_transcript() {
        let secret = random_pairing_secret().expect("secret should generate");
        let proof = pairing_proof(&secret, b"first");
        verify_pairing_proof(&secret, b"first", &proof).expect("proof should verify");
        assert!(verify_pairing_proof(&secret, b"second", &proof).is_err());
    }
}
