use anyhow::{anyhow, Result};
use alloy_primitives::Address;
use kms_api::kms::v1::{
    CrsGenRequest, FheParameter, KeyGenPreprocRequest, KeyGenRequest, PublicDecryptionRequest,
    TypedCiphertext, TypedPlaintext, UserDecryptionRequest,
};
use kms_api::{KeyId, RequestId};
use kms_core_client::{
    encrypt, CipherParameters, EncryptionResult, SharedKeyGenParameters,
};
use kms_lib::client::{
    client_wasm::Client,
    user_decryption_wasm::ParsedUserDecryptionRequest,
};
use kms_lib::cryptography::internal_crypto_types::{ UnifiedPrivateEncKey, UnifiedPublicEncKey };
use std::path::Path;

use crate::decrypt::{
    poll_public_decrypt_result, poll_user_decrypt_result, submit_public_decrypt,
    submit_user_decrypt,
};
use crate::encrypt::materialize_key_for_encryption;
use crate::pylon_client::MeshGatewayClient;
use crate::key_material::{
    poll_crsgen_result,
    poll_keygen_preproc_result, poll_keygen_result, submit_crsgen, submit_keygen,
    submit_keygen_preproc,
};
use crate::materialize::{materialize_key_bundle, materialize_crs_bundle};
use crate::verify::{
    reconstruct_user_decrypt_aggregate, verify_public_decrypt_aggregate,
    UserDecryptReconstructionContext,
};

/// A Mesh-side orchestration bridge that replaces the old gRPC orchestration path
/// for Pylon-mediated Mesh A -> Mesh B flows.
///
/// This object is intentionally state-light:
/// - it owns the Face-A gateway client
/// - it borrows an initialized internal `kms_lib` client for request creation and
///   verification/reconstruction
///
/// Higher-level CLI glue (node_kentr, autonomos, etc.) can keep one of these around
/// and call operation-specific methods.
pub struct MeshCoreClientBridge {
    pub gateway: MeshGatewayClient,
    pub internal_client: Client,
    pub kms_addrs: Vec<Address>,
}

impl MeshCoreClientBridge {
    pub fn new(
        gateway: MeshGatewayClient,
        internal_client: Client,
        kms_addrs: Vec<Address>,
    ) -> Self {
        Self {
            gateway,
            internal_client,
            kms_addrs,
        }
    }

    /// Submit keygen preprocessing and wait for aggregated per-party results.
    ///
    /// This preserves the old semantic:
    /// - request is constructed by the internal KMS client
    /// - request id is caller-owned
    /// - result polling is done through Pylon
    pub async fn do_preproc_keygen(
        &mut self,
        request_id: RequestId,
        req: KeyGenPreprocRequest,
    ) -> Result<RequestId> {
        let meta = default_job_metadata("keygen-preproc");
        submit_keygen_preproc(&mut self.gateway, request_id.into_bytes(), req, meta).await?;

        let agg = poll_keygen_preproc_result(&mut self.gateway, request_id.into_bytes())
            .await?
            .ok_or_else(|| anyhow!("timeout while waiting for keygen preproc result"))?;

        for resp in agg.responses {
            self.internal_client.process_preproc_response(
                &request_id,
                &dummy_domain(),
                &resp,
            )?;
        }

        Ok(request_id)
    }

    /// Submit keygen, poll, materialize, and verify the produced public artifacts.
        /// Submit keygen, poll, materialize, and verify the produced public artifacts.
    pub async fn do_keygen(
        &mut self,
        cache_root: &Path,
        request_id: RequestId,
        preproc_id: RequestId,
        req: KeyGenRequest,
        party_id_for_load: usize,
    ) -> Result<RequestId> {
        let _domain = req
            .domain
            .as_ref()
            .ok_or_else(|| anyhow!("keygen request missing domain"))
            .and_then(kms_api::rpc_types::protobuf_to_alloy_domain)?;

        let _ = preproc_id;
        let _ = party_id_for_load;
        let _ = cache_root;

        let meta = default_job_metadata("keygen");
        submit_keygen(&mut self.gateway, request_id.into_bytes(), req, meta).await?;

        let _manifest_cid = poll_keygen_result(&mut self.gateway, request_id.into_bytes())
            .await?
            .ok_or_else(|| anyhow!("timeout while waiting for keygen result"))?;

        materialize_key_bundle(&mut self.gateway, request_id.into_bytes()).await?;

        Ok(request_id)
    }

    /// Submit CRS generation, poll, materialize, and verify.
        /// Submit CRS generation, poll, materialize, and verify.
    pub async fn do_crsgen(
        &mut self,
        cache_root: &Path,
        request_id: RequestId,
        req: CrsGenRequest,
        party_id_for_load: usize,
    ) -> Result<RequestId> {
        let _domain = req
            .domain
            .as_ref()
            .ok_or_else(|| anyhow!("crsgen request missing domain"))
            .and_then(kms_api::rpc_types::protobuf_to_alloy_domain)?;

        let _ = party_id_for_load;
        let _ = cache_root;

        let meta = default_job_metadata("crsgen");
        submit_crsgen(&mut self.gateway, request_id.into_bytes(), req, meta).await?;

        let _manifest_cid = poll_crsgen_result(&mut self.gateway, request_id.into_bytes())
            .await?
            .ok_or_else(|| anyhow!("timeout while waiting for crsgen result"))?;

        materialize_crs_bundle(&mut self.gateway, request_id.into_bytes()).await?;

        Ok(request_id)
    }

    /// Local encryption using public material fetched through Pylon.
    ///
    /// This keeps the old local-encrypt semantics while replacing public-material
    /// retrieval.
    pub async fn do_encrypt(
        &mut self,
        cache_root: &Path,
        party_id_for_load: usize,
        params: CipherParameters,
    ) -> Result<EncryptionResult, Box<dyn std::error::Error + 'static>> {
        materialize_key_for_encryption(&mut self.gateway, params.key_id.into_bytes()).await?;
        encrypt(cache_root, party_id_for_load, params).await
    }

    /// Submit a public decrypt request, poll the aggregate, and verify it.
    pub async fn do_public_decrypt(
        &mut self,
        req: PublicDecryptionRequest,
        expected_answer: Option<TypedPlaintext>,
    ) -> Result<Vec<kms_api::kms::v1::PublicDecryptionResponse>> {
        let request_id: RequestId = req
            .request_id
            .clone()
            .ok_or_else(|| anyhow!("public decrypt request missing request_id"))?
            .try_into()?;

        let meta = default_job_metadata("public-decrypt");
        submit_public_decrypt(&mut self.gateway, request_id.into_bytes(), req.clone(), meta).await?;

        let agg = poll_public_decrypt_result(&mut self.gateway, request_id.into_bytes())
            .await?
            .ok_or_else(|| anyhow!("timeout while waiting for public decrypt result"))?;

        verify_public_decrypt_aggregate(
            &self.internal_client,
            Some(&req),
            &agg,
            expected_answer,
            &self.kms_addrs,
        )
    }

    /// Submit a user decrypt request, poll the aggregate, and reconstruct plaintexts.
    ///
    /// The caller must provide the exact ML-KEM keypair + parsed request corresponding
    /// to the original request construction.
    pub async fn do_user_decrypt(
        &mut self,
        req: UserDecryptionRequest,
        parsed_request: ParsedUserDecryptionRequest,
        enc_pk: UnifiedPublicEncKey,
        enc_sk: UnifiedPrivateEncKey,
    ) -> Result<Vec<TypedPlaintext>> {
        let request_id: RequestId = req
            .request_id
            .clone()
            .ok_or_else(|| anyhow!("user decrypt request missing request_id"))?
            .try_into()?;

        let meta = default_job_metadata("user-decrypt");
        submit_user_decrypt(&mut self.gateway, request_id.into_bytes(), req.clone(), meta).await?;

        let agg = poll_user_decrypt_result(&mut self.gateway, request_id.into_bytes())
            .await?
            .ok_or_else(|| anyhow!("timeout while waiting for user decrypt result"))?;

        let ctx = UserDecryptReconstructionContext {
            request: req,
            parsed_request,
            enc_pk,
            enc_sk,
        };

        reconstruct_user_decrypt_aggregate(&self.internal_client, &agg, &ctx)
    }
}

/// Small helper for bridge callers that want the old request-building path.
///
/// This uses the internal KMS client to build a keygen-preproc request, then the
/// bridge can submit it over Mesh/Pylon.
pub fn build_preproc_request(
    internal_client: &mut Client,
    request_id: &RequestId,
    param: FheParameter,
) -> Result<KeyGenPreprocRequest> {
    internal_client
        .preproc_request(request_id, Some(param), None, &dummy_domain())
        .map_err(|e| anyhow!("failed to build keygen preproc request: {e}"))
}

/// Small helper for Mesh-side keygen request construction via the existing KMS client.
pub fn build_keygen_request(
    internal_client: &mut Client,
    request_id: &RequestId,
    preproc_id: &RequestId,
    param: FheParameter,
    shared: &SharedKeyGenParameters,
) -> Result<KeyGenRequest> {
    let keyset_config = shared
        .keyset_type
        .clone()
        .map(|x| kms_api::kms::v1::KeySetConfig {
            keyset_type: kms_api::kms::v1::KeySetType::from(x) as i32,
            standard_keyset_config: None,
        });

    let keyset_added_info = shared
        .keyset_added_info
        .clone()
        .map(kms_api::kms::v1::KeySetAddedInfo::from);

    internal_client
        .key_gen_request(
            request_id,
            preproc_id,
            Some(param),
            keyset_config,
            keyset_added_info,
            dummy_domain(),
        )
        .map_err(|e| anyhow!("failed to build keygen request: {e}"))
}

/// Small helper for Mesh-side CRS request construction via the existing KMS client.
pub fn build_crsgen_request(
    internal_client: &mut Client,
    request_id: &RequestId,
    max_num_bits: Option<u32>,
    param: FheParameter,
) -> Result<CrsGenRequest> {
    internal_client
        .crs_gen_request(request_id, max_num_bits, Some(param), &dummy_domain())
        .map_err(|e| anyhow!("failed to build crsgen request: {e}"))
}

/// Helper to build a public decryption request through the existing KMS client.
///
/// This is the preferred path because it preserves:
/// - domain
/// - external handles
/// - exact request structure expected by KMS verification logic
pub fn build_public_decrypt_request(
    internal_client: &mut Client,
    ciphertexts: Vec<TypedCiphertext>,
    request_id: &RequestId,
    key_id: &KeyId,
) -> Result<PublicDecryptionRequest> {
    let key_req_id: RequestId = (*key_id).into();

    internal_client
        .public_decryption_request(
            ciphertexts,
            &dummy_domain(),
            request_id,
            &key_req_id,
        )
        .map_err(|e| anyhow!("failed to build public decrypt request: {e}"))
}

/// Helper to build a user decryption request through the existing KMS client.
///
/// Returns:
/// - protobuf request
/// - encapsulation public key
/// - encapsulation private key
pub fn build_user_decrypt_request(
    internal_client: &mut Client,
    ciphertexts: Vec<TypedCiphertext>,
    request_id: &RequestId,
    key_id: &KeyId,
) -> Result<(
    UserDecryptionRequest,
    ParsedUserDecryptionRequest,
    UnifiedPublicEncKey,
    UnifiedPrivateEncKey,
)> {
    let key_req_id: RequestId = key_id_as_request_id(key_id);

    let (req, enc_pk, enc_sk) = internal_client
        .user_decryption_request(
            &dummy_domain(),
            ciphertexts,
            request_id,
            &key_req_id,
        )
        .map_err(|e| anyhow!("failed to build user decrypt request: {e}"))?;

    let parsed = ParsedUserDecryptionRequest::try_from(&req)
        .map_err(|e| anyhow!("failed to parse user decrypt request: {e}"))?;

    Ok((req, parsed, enc_pk, enc_sk))
}

fn default_job_metadata(client_hint: &str) -> mesh_gateway_wire::JobMetadata {
    mesh_gateway_wire::JobMetadata {
        created_unix_ms: now_unix_ms(),
        client_hint: Some(client_hint.to_string()),
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn dummy_domain() -> alloy_sol_types::Eip712Domain {
    alloy_sol_types::eip712_domain!(
        name: "Authorization token",
        version: "1",
        chain_id: 8006,
        verifying_contract: alloy_primitives::address!("66f9664f97F2b50F62D13eA064982f936dE76657"),
    )
}

fn key_id_as_request_id(key_id: &KeyId) -> RequestId {
    (*key_id).into()
}