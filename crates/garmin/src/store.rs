//! Ablage der Token (SPEC 5.6, 10.2): Schlüsselbund des Betriebssystems, für Tests im Speicher.
//!
//! Token stehen nie in einer Datei der App. Der Eintrag heißt Dienst `lastwerk-garmin`, Konto
//! `default`. Fehler tragen keinen Inhalt: Die Fehler des Schlüsselbund-Crates können die
//! gelesenen Bytes enthalten (`BadEncoding`), deshalb werden sie auf `StoreError` abgebildet,
//! bevor sie das Crate verlassen.

use std::sync::Mutex;

use crate::auth::{StoredTokens, TokenSet};

/// Dienstname im Schlüsselbund.
pub const KEYRING_SERVICE: &str = "lastwerk-garmin";
/// Kontoname im Schlüsselbund.
pub const KEYRING_ACCOUNT: &str = "default";

/// Größe, ab der der Zugangstoken nicht mehr mit abgelegt wird. Die Windows-Anmeldeinformations-
/// verwaltung nimmt höchstens 2560 Bytes je Eintrag (`CRED_MAX_CREDENTIAL_BLOB_SIZE`, Quelle:
/// Quelltext von `windows-native-keyring-store`); 2400 lässt Spielraum. Das Erneuerungstoken ist
/// klein, der Zugangstoken (ein JWT) kann über 1000 Bytes erreichen. Fehlt er, erneuert der Client
/// vor der ersten Anfrage, das kostet einen Aufruf je Sitzung.
const KEYRING_MAX_BYTES: usize = 2400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// Schlüsselbund nicht erreichbar oder gesperrt (unter Linux: kein Secret Service).
    #[error("credential store unavailable")]
    Unavailable,
    /// Eintrag vorhanden, aber nicht lesbar.
    #[error("stored credentials are unreadable")]
    Corrupt,
    /// Eintrag passt nicht in den Schlüsselbund.
    #[error("credentials too large for the store")]
    TooLarge,
    #[error("credential store error")]
    Other,
}

pub trait TokenStore: Send + Sync {
    /// `Ok(None)`, wenn nichts abgelegt ist.
    fn load(&self) -> Result<Option<TokenSet>, StoreError>;
    fn save(&self, tokens: &TokenSet) -> Result<(), StoreError>;
    /// Löscht den Eintrag. Ein fehlender Eintrag ist kein Fehler.
    fn delete(&self) -> Result<(), StoreError>;
}

/// Ablage im Speicher für Tests. Kein Inhalt in `Debug`.
#[derive(Default)]
pub struct MemoryStore {
    tokens: Mutex<Option<TokenSet>>,
}

impl std::fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemoryStore { .. }")
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tokens(tokens: TokenSet) -> Self {
        Self {
            tokens: Mutex::new(Some(tokens)),
        }
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, Option<TokenSet>> {
        self.tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl TokenStore for MemoryStore {
    fn load(&self) -> Result<Option<TokenSet>, StoreError> {
        Ok(self.slot().clone())
    }

    fn save(&self, tokens: &TokenSet) -> Result<(), StoreError> {
        *self.slot() = Some(tokens.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), StoreError> {
        *self.slot() = None;
        Ok(())
    }
}

/// Schlüsselbund des Betriebssystems über das Crate `keyring` (Funktion `v1`):
/// Windows-Anmeldeinformationsverwaltung, macOS-Schlüsselbund, Secret Service unter Linux.
///
/// Einschränkung Linux: Ohne laufenden Secret Service (GNOME-Schlüsselbund, KWallet) schlagen
/// `load`, `save` und `delete` mit `Unavailable` fehl, und es gibt keinen Ersatz im Klartext.
/// Das Crate braucht dafür keine Systembibliothek (reines Rust über `zbus`), die CI baut also
/// ohne neues Paket. Der Kernel-Schlüsselbund (`linux-native`) bleibt aus, weil er einen Neustart
/// nicht überlebt.
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: String,
    account: String,
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyringStore {
    pub fn new() -> Self {
        Self::with_names(KEYRING_SERVICE, KEYRING_ACCOUNT)
    }

    /// Eigene Namen, damit ein Livetest den echten Eintrag nicht berührt.
    pub fn with_names(service: &str, account: &str) -> Self {
        Self {
            service: service.to_owned(),
            account: account.to_owned(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, StoreError> {
        keyring::Entry::new(&self.service, &self.account).map_err(map_keyring_error)
    }
}

impl TokenStore for KeyringStore {
    fn load(&self) -> Result<Option<TokenSet>, StoreError> {
        match self.entry()?.get_secret() {
            Ok(bytes) => decode(&bytes).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(map_keyring_error(err)),
        }
    }

    fn save(&self, tokens: &TokenSet) -> Result<(), StoreError> {
        // Bytes statt Zeichenkette: Windows legt Zeichenketten als UTF-16 ab und verdoppelt
        // damit die Größe gegenüber dem Limit.
        let bytes = encode(tokens)?;
        self.entry()?.set_secret(&bytes).map_err(map_keyring_error)
    }

    fn delete(&self) -> Result<(), StoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(map_keyring_error(err)),
        }
    }
}

/// Bildet Fehler des Schlüsselbund-Crates auf `StoreError` ab, ohne Meldung und ohne Bytes.
fn map_keyring_error(err: keyring::Error) -> StoreError {
    use keyring::Error as E;
    match err {
        E::NoStorageAccess(_)
        | E::PlatformFailure(_)
        | E::NoDefaultStore
        | E::NotSupportedByStore(_) => StoreError::Unavailable,
        E::BadEncoding(_) | E::BadDataFormat(..) => StoreError::Corrupt,
        E::TooLong(..) => StoreError::TooLarge,
        _ => StoreError::Other,
    }
}

/// JSON für den Schlüsselbund. Passt der Eintrag mit Zugangstoken nicht unter die Grenze, wird
/// er ohne ihn abgelegt; passt er auch dann nicht, `TooLarge`.
pub(crate) fn encode(tokens: &TokenSet) -> Result<Vec<u8>, StoreError> {
    for with_access_token in [true, false] {
        let bytes = serde_json::to_vec(&tokens.to_stored(with_access_token))
            .map_err(|_| StoreError::Other)?;
        if bytes.len() <= KEYRING_MAX_BYTES {
            return Ok(bytes);
        }
    }
    Err(StoreError::TooLarge)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<TokenSet, StoreError> {
    let stored: StoredTokens = serde_json::from_slice(bytes).map_err(|_| StoreError::Corrupt)?;
    TokenSet::from_stored(stored).ok_or(StoreError::Corrupt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryStore::new();
        assert_eq!(store.load(), Ok(None));
        let tokens = TokenSet::for_tests("a", Some("r"), "cid");
        store.save(&tokens).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), tokens);
        store.delete().unwrap();
        assert_eq!(store.load(), Ok(None));
        // Löschen ohne Eintrag ist kein Fehler.
        store.delete().unwrap();
    }

    #[test]
    fn encode_keeps_access_token_when_it_fits() {
        let tokens = TokenSet::for_tests(&"a".repeat(1000), Some("refresh"), "cid");
        let back = decode(&encode(&tokens).unwrap()).unwrap();
        assert_eq!(back, tokens);
    }

    // Zugangstoken von 2500 Bytes ist zu groß; Erneuerungstoken und Kennung bleiben (rund 60 Bytes).
    #[test]
    fn encode_drops_oversized_access_token() {
        let tokens = TokenSet::for_tests(&"a".repeat(2500), Some("refresh"), "cid");
        let bytes = encode(&tokens).unwrap();
        assert!(bytes.len() < 100, "{}", bytes.len());
        let back = decode(&bytes).unwrap();
        assert_eq!(back.access_token_for_tests(), "");
        assert_eq!(back.refresh_token_for_tests(), Some("refresh"));
        assert_eq!(back.client_id(), "cid");
    }

    #[test]
    fn encode_fails_when_even_the_refresh_token_is_too_large() {
        let tokens = TokenSet::for_tests("a", Some(&"r".repeat(3000)), "cid");
        assert_eq!(encode(&tokens), Err(StoreError::TooLarge));
    }

    #[test]
    fn decode_rejects_garbage_without_leaking_it() {
        for bad in [&b"not json"[..], b"{}", br#"{"v":1}"#, b"", b"\xff\xfe"] {
            assert_eq!(decode(bad).unwrap_err(), StoreError::Corrupt);
        }
    }

    #[test]
    fn keyring_errors_map_without_payload() {
        use keyring::Error as E;
        assert_eq!(
            map_keyring_error(E::NoDefaultStore),
            StoreError::Unavailable
        );
        assert_eq!(
            map_keyring_error(E::BadEncoding(b"secret-bytes".to_vec())),
            StoreError::Corrupt
        );
        assert_eq!(
            map_keyring_error(E::TooLong("secret".into(), 2560)),
            StoreError::TooLarge
        );
        assert_eq!(
            map_keyring_error(E::Invalid("a".into(), "b".into())),
            StoreError::Other
        );
        assert_eq!(
            map_keyring_error(E::NotSupportedByStore("x".into())),
            StoreError::Unavailable
        );
    }

    /// Livetest gegen den echten Schlüsselbund dieses Rechners. Läuft nur mit
    /// `cargo test -p lastwerk-garmin -- --ignored` und mit erreichbarem Schlüsselbund (nicht in
    /// der CI). Eigene Namen, der echte Eintrag bleibt unberührt.
    #[test]
    #[ignore = "braucht einen Schlüsselbund des Betriebssystems"]
    fn keyring_round_trip_live() {
        let store = KeyringStore::with_names("lastwerk-garmin-test", "t21");
        let tokens = TokenSet::for_tests("synthetic-access", Some("synthetic-refresh"), "cid");
        store.save(&tokens).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), tokens);
        store.delete().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
