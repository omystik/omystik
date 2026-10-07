// fhe-cipher/src/lib.rs

use anyhow::{anyhow, Context, Result};
pub use kms_api::kms::v1::{CiphertextFormat, TypedCiphertext, TypedPlaintext};

use std::{io::Cursor, path::Path};

use tfhe::{
    CompactCiphertextList, CompactPublicKey,
    safe_serialization::{safe_deserialize, safe_serialize},
};
use tfhe::integer::bigint::U256;

/// Local extension until signed types are represented in the KMS FheType enum.
pub const FHE_INT64: i32 = 10_064;

/// Same numeric value as kms_core_client::FheType::Euint64 / tfhe FheTypes::Uint64.
pub const FHE_UINT64: i32 = 5;

/// Same numeric value as KMS / TFHE ebool.
pub const FHE_BOOL: i32 = 0;

pub const COMPACT_LIST_ENCODING_V1: &str = "tfhe.safe_serialize.CompactCiphertextList.v1";

const SAFE_SER_SIZE_LIMIT: u64 = 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct FheEncryptor {
    public_key: CompactPublicKey,
}

impl FheEncryptor {
    pub fn from_public_key(public_key: CompactPublicKey) -> Self {
        Self { public_key }
    }

    pub async fn from_public_key_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("failed to read PublicKey at {}", path.display()))?;

        Self::from_public_key_bytes(&bytes)
            .with_context(|| format!("failed to decode CompactPublicKey at {}", path.display()))
    }

    pub fn from_public_key_bytes(bytes: &[u8]) -> Result<Self> {
        let public_key = decode_public_key(bytes)?;
        Ok(Self { public_key })
    }

    pub fn encrypt_typed_plaintext(&self, plaintext: TypedPlaintext) -> Result<TypedCiphertext> {
        let ciphertext =
            encrypt_typed_bytes(&self.public_key, plaintext.fhe_type, &plaintext.bytes)?;

        Ok(TypedCiphertext {
            ciphertext,
            fhe_type: plaintext.fhe_type,
            external_handle: vec![],

            // The bytes are CompactCiphertextList, but the current KMS protobuf enum
            // has no CompactCiphertextList discriminant. Do not rely on this field
            // for smart-program input decoding; rely on the program ABI.
            ciphertext_format: CiphertextFormat::SmallExpanded as i32,
        })
    }

    pub fn encrypt_many(
        &self,
        plaintexts: impl IntoIterator<Item = TypedPlaintext>,
    ) -> Result<Vec<TypedCiphertext>> {
        plaintexts
            .into_iter()
            .map(|plaintext| self.encrypt_typed_plaintext(plaintext))
            .collect()
    }

    pub fn encrypt_bool(&self, value: bool) -> Result<Vec<u8>> {
        Ok(self
            .encrypt_typed_plaintext(TypedPlaintext {
                bytes: vec![u8::from(value)],
                fhe_type: FHE_BOOL,
            })?
            .ciphertext)
    }

    pub fn encrypt_u64(&self, value: u64) -> Result<Vec<u8>> {
        Ok(self
            .encrypt_typed_plaintext(TypedPlaintext {
                bytes: value.to_le_bytes().to_vec(),
                fhe_type: FHE_UINT64,
            })?
            .ciphertext)
    }

    pub fn encrypt_i64(&self, value: i64) -> Result<Vec<u8>> {
        Ok(self
            .encrypt_typed_plaintext(TypedPlaintext {
                bytes: value.to_le_bytes().to_vec(),
                fhe_type: FHE_INT64,
            })?
            .ciphertext)
    }
}

pub fn decode_public_key(bytes: &[u8]) -> Result<CompactPublicKey> {
    let cursor = Cursor::new(bytes);

    safe_deserialize::<CompactPublicKey>(cursor, SAFE_SER_SIZE_LIMIT)
        .map_err(|e| anyhow!("safe_deserialize CompactPublicKey failed: {e}"))
}

pub fn encode_compact_ciphertext_list(value: &CompactCiphertextList) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();

    safe_serialize(value, &mut bytes, SAFE_SER_SIZE_LIMIT)
        .map_err(|e| anyhow!("safe_serialize CompactCiphertextList failed: {e}"))?;

    Ok(bytes)
}

fn encrypt_typed_bytes(
    public_key: &CompactPublicKey,
    fhe_type: i32,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let list = match fhe_type {
        // ebool
        0 => {
            let value = decode_bool(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint4
        1 => {
            let value = decode_uint_le::<u8>(bytes)? & 0x0f;
            build_compact_list(public_key, |builder| {
                builder
                    .push_with_num_bits(value, 4)
                    .map_err(|e| anyhow!("compact encrypt euint4 failed: {e}"))?;
                Ok(())
            })?
        }

        // euint8
        2 => {
            let value = decode_uint_le::<u8>(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint16
        3 => {
            let value = decode_uint_le::<u16>(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint32
        4 => {
            let value = decode_uint_le::<u32>(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint64
        5 => {
            let value = decode_uint_le::<u64>(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint128
        6 => {
            let value = decode_uint_le::<u128>(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        // euint256
        8 => {
            let value = decode_u256_le(bytes)?;
            build_compact_list(public_key, |builder| {
                builder
                    .push_with_num_bits(value, 256)
                    .map_err(|e| anyhow!("compact encrypt euint256 failed: {e}"))?;
                Ok(())
            })?
        }

        // local signed i64 extension
        FHE_INT64 => {
            let value = decode_i64_le(bytes)?;
            build_compact_list(public_key, |builder| {
                builder.push(value);
                Ok(())
            })?
        }

        other => {
            return Err(anyhow!(
                "fhe-cipher encryptor does not support fhe_type={other} yet"
            ));
        }
    };

    encode_compact_ciphertext_list(&list)
}

fn build_compact_list<F>(
    public_key: &CompactPublicKey,
    f: F,
) -> Result<CompactCiphertextList>
where
    F: FnOnce(&mut tfhe::CompactCiphertextListBuilder) -> Result<()>,
{
    let mut builder = CompactCiphertextList::builder(public_key);
    f(&mut builder)?;
    Ok(builder.build())
}

fn decode_bool(bytes: &[u8]) -> Result<bool> {
    match bytes {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(anyhow!(
            "bool plaintext expects exactly one byte equal to 0 or 1, got {} bytes",
            bytes.len()
        )),
    }
}

fn decode_i64_le(bytes: &[u8]) -> Result<i64> {
    if bytes.len() > 8 {
        return Err(anyhow!(
            "i64 plaintext expects at most 8 little-endian bytes, got {}",
            bytes.len()
        ));
    }

    let mut buf = [0u8; 8];
    buf[..bytes.len()].copy_from_slice(bytes);

    Ok(i64::from_le_bytes(buf))
}

fn decode_uint_le<T>(bytes: &[u8]) -> Result<T>
where
    T: TryFrom<u128>,
    <T as TryFrom<u128>>::Error: std::fmt::Debug,
{
    if bytes.len() > 16 {
        return Err(anyhow!(
            "integer plaintext too large for scalar decode: {} bytes",
            bytes.len()
        ));
    }

    let mut buf = [0u8; 16];
    let n = bytes.len().min(16);
    buf[..n].copy_from_slice(&bytes[..n]);

    let value = u128::from_le_bytes(buf);

    T::try_from(value).map_err(|_| anyhow!("integer plaintext does not fit target FHE type"))
}

fn decode_u256_le(bytes: &[u8]) -> Result<U256> {
    if bytes.len() > 32 {
        return Err(anyhow!(
            "euint256 plaintext expects at most 32 little-endian bytes, got {}",
            bytes.len()
        ));
    }

    let mut padded = [0u8; 32];
    padded[..bytes.len()].copy_from_slice(bytes);

    let mut value = U256::from(0u64);
    value.copy_from_le_byte_slice(&padded);

    Ok(value)
}