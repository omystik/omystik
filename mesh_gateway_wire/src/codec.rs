use anyhow::Result;
use bc2wrap::{serialize, deserialize_safe};
use serde::{Serialize, de::DeserializeOwned};

/// Serialize a value into a binary representation.
pub fn encode<T: Serialize>(t: &T) -> Result<Vec<u8>> { Ok(serialize(t)?) }

/// Deserialize a value from a binary representation.
pub fn decode<T: DeserializeOwned>(b: &[u8]) -> Result<T> { Ok(deserialize_safe::<T>(b)?) }
