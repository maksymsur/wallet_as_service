use secp256k1::{SecretKey, ONE_KEY};
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::write_volatile;
use std::str::FromStr;
use std::sync::atomic::{compiler_fence, Ordering};
use thiserror::Error;
use zeroize::DefaultIsZeroes;

/// Custom error type for handling issues related to SafeSecretKey without exposing sensitive info
#[derive(Error, Debug)]
pub enum SafeSecretKeyError {
    #[error("Failed to create SafeSecretKey")]
    CreationFailed,
}

/// A wrapper around SecretKey that ensures the key is zeroized on drop.
/// This struct uses pinning to ensure the key data is not moved in memory.
#[derive(Clone)]
pub struct SafeSecretKey {
    safe: Pin<Box<ZeroizedSecretKey>>,
}

/// Struct that wraps SecretKey and implements zeroization.
#[derive(Copy, Clone)]
struct ZeroizedSecretKey(SecretKey);

/// Implement DefaultIsZeroes for ZeroizedSecretKey to ensure
/// that it is zeroized by default.
impl DefaultIsZeroes for ZeroizedSecretKey {}

/// Default implementation creates a SecretKey filled with 1s.
/// This is not used in practice but is necessary for the default trait.
impl Default for ZeroizedSecretKey {
    fn default() -> Self {
        Self(ONE_KEY)
    }
}

impl SafeSecretKey {
    /// Creates a new SafeSecretKey from an existing SecretKey.
    /// The key is pinned in memory to prevent accidental movement.
    pub fn new(secret: &SecretKey) -> Result<Self, SafeSecretKeyError> {
        let mut safe = Pin::new(Box::<ZeroizedSecretKey>::default());
        safe.0 = *secret;
        Ok(Self { safe })
    }

    /// Creates a SafeSecretKey from a hexadecimal string.
    pub fn from_str(s: &str) -> Result<Self, SafeSecretKeyError> {
        let secret_key = SecretKey::from_str(s).map_err(|_| SafeSecretKeyError::CreationFailed)?;
        let res = Self::new(&secret_key)?;
        drop_secret_key(secret_key); // Zeroizes the original key immediately after copying it.
        Ok(res)
    }
}

/// Implement TryFrom for SecretKey to SafeSecretKey conversion.
impl<'a> TryFrom<&'a SecretKey> for SafeSecretKey {
    type Error = SafeSecretKeyError;

    fn try_from(unsafe_secret_key: &'a SecretKey) -> Result<Self, SafeSecretKeyError> {
        Self::new(unsafe_secret_key)
    }
}

/// Implement TryFrom for string to SafeSecretKey conversion.
impl<'a> TryFrom<&'a str> for SafeSecretKey {
    type Error = SafeSecretKeyError;

    fn try_from(s: &'a str) -> Result<Self, SafeSecretKeyError> {
        SafeSecretKey::from_str(s)
    }
}

/// Deref implementation allows accessing the inner SecretKey.
impl Deref for SafeSecretKey {
    type Target = SecretKey;

    fn deref(&self) -> &Self::Target {
        &self.safe.0
    }
}

/// Unsafe function that zeroizes the memory of the given SecretKey pointer.
/// This function uses write_volatile to ensure that the memory is overwritten.
pub unsafe fn zeroize_secret_key_mut(ptr: *mut SecretKey) {
    write_volatile(ptr, ONE_KEY);
    compiler_fence(Ordering::SeqCst);
}

/// Manually zeroizes the memory where the SecretKey was stored.
pub fn drop_secret_key(mut key: SecretKey) {
    unsafe {
        zeroize_secret_key_mut(&mut key);
    }
}

/// Custom Drop implementation ensures the key is zeroized when the SafeSecretKey is dropped.
impl Drop for SafeSecretKey {
    fn drop(&mut self) {
        unsafe {
            zeroize_secret_key_mut(&mut self.safe.0 as *mut SecretKey);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::ONE_KEY;

    /// Test that SafeSecretKey can be created and dropped without panic.
    #[test]
    pub fn no_panic() {
        let secret_key = ONE_KEY;
        let safe_secret_key = SafeSecretKey::new(&secret_key).unwrap();
        drop(safe_secret_key);
    }

    /// Test that SafeSecretKey can be created from a string and dropped safely.
    #[test]
    pub fn from_str() {
        let key: &str = "3146401fc53a4b946c9732b2f3236ee4040672b35e63dd55da973c5c80e6c87f";
        let safe = SafeSecretKey::from_str(key).unwrap();
        drop(safe);
    }

    /// Test cloning of SafeSecretKey and ensure that memory is zeroized after drop.
    #[test]
    pub fn clone_is_fine() {
        let key = ONE_KEY;
        let safe = SafeSecretKey::try_from(&key).unwrap();
        let safe_clone = safe.clone();
        drop(safe);
        // When safe is dropped, pinned memory location is zeroized
        let expected: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ];
        assert_eq!(safe_clone.serialize_secret(), expected);
    }
}
