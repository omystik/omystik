use anyhow::Result;
pub use kryptografia::{
    TypedCiphertext,
    TypedPlaintext,
    FHE_INT64 as AUTONOMOS_FHE_INT64,
    FHE_UINT64 as AUTONOMOS_FHE_UINT64,
    FheEncryptor,
};
use libp2p_common::{key_ops::load_local_key, node_primary::KeyType};

#[derive(Clone)]
pub struct AutonomosEncryptor {
    inner: FheEncryptor,
}

impl AutonomosEncryptor {
    pub async fn from_local_public_key() -> Result<Self> {
        let key_path = load_local_key(Some(KeyType::PublicKey)).await?;
        Ok(Self {
            inner: FheEncryptor::from_public_key_path(key_path).await?,
        })
    }

    pub async fn from_public_key_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(Self {
            inner: FheEncryptor::from_public_key_path(path).await?,
        })
    }

    pub async fn encrypt_typed_plaintext(
        &self,
        plaintext: TypedPlaintext,
    ) -> Result<TypedCiphertext> {
        self.inner.encrypt_typed_plaintext(plaintext)
    }

    pub async fn encrypt_many(
        &self,
        plaintexts: impl IntoIterator<Item = TypedPlaintext>,
    ) -> Result<Vec<TypedCiphertext>> {
        self.inner.encrypt_many(plaintexts)
    }
}