use anyhow::{anyhow, Result};
use prost::Message;
use libp2p::PeerId;
use kms_api::kms::v1::{
    PublicDecryptionRequest, PublicDecryptionResponse, TypedCiphertext, UserDecryptionRequest,
    UserDecryptionResponse,
};
use mesh_gateway_wire::{AggregatedJobResult, JobId, JobResultPayload, JobMetadata, JobType, Payload};

use crate::pylon_client::{MeshGatewayClient, ResolvedJobResult};

/// Decoded aggregated public decryption result.
///
/// `responses` are the per-party protobuf responses, in the deterministic order
/// preserved by the gateway aggregate.
#[derive(Debug, Clone)]
pub struct PublicDecryptAggregate {
    pub job_id: JobId,
    pub responses: Vec<PublicDecryptionResponse>,
}

/// Decoded aggregated user decryption result.
///
/// `responses` are the per-party protobuf responses, in the deterministic order
/// preserved by the gateway aggregate.
#[derive(Debug, Clone)]
pub struct UserDecryptAggregate {
    pub job_id: JobId,
    pub responses: Vec<UserDecryptionResponse>,
}

/// Submit a public decrypt job through Pylon.
///
/// The request bytes remain the original protobuf message, matching the current
/// Face B executor contract in KMS.
pub async fn submit_public_decrypt(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
    request: PublicDecryptionRequest,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(request.encode_to_vec());
    gateway
        .submit_job(job_id, JobType::PublicDecrypt, payload, metadata)
        .await
}

/// Submit a user decrypt job through Pylon.
///
/// The request bytes remain the original protobuf message, matching the current
/// Face B executor contract in KMS.
pub async fn submit_user_decrypt(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
    request: UserDecryptionRequest,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(request.encode_to_vec());
    gateway
        .submit_job(job_id, JobType::UserDecrypt, payload, metadata)
        .await
}

/// Poll and decode an aggregated public decrypt result.
///
/// The gateway is expected to return a `Payload::BlobRef` or `Payload::Inline`
/// containing a `mesh_gateway_wire::AggregatedJobResult`.
pub async fn poll_public_decrypt_result(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
) -> Result<Option<PublicDecryptAggregate>> {
    let (pylon_peer, result) = gateway.wait_job_result_resolved(job_id).await?;
    
    let payload = match result {
        ResolvedJobResult::Payload(p) => p,
        ResolvedJobResult::PublishedManifest { .. } => {
            return Err(anyhow!("decrypt expected payload result, got manifest"));
        }
    };
    let agg = decode_aggregated_result_from_peer(gateway, pylon_peer, payload).await?;
    
    if !matches!(agg.job_type, JobType::PublicDecrypt) {
        return Err(anyhow!(
            "job type mismatch: expected PublicDecrypt, got {:?}",
            agg.job_type
        ));
    }

    let mut responses = Vec::with_capacity(agg.proto_results.len());
    for entry in agg.proto_results {
        let resp = PublicDecryptionResponse::decode(entry.bytes.as_slice())
            .map_err(|e| anyhow!("failed to decode PublicDecryptionResponse: {e}"))?;
        responses.push(resp);
    }

    Ok(Some(PublicDecryptAggregate {
        job_id: agg.job_id,
        responses,
    }))
}

/// Poll and decode an aggregated user decrypt result.
///
/// The gateway is expected to return a `Payload::BlobRef` or `Payload::Inline`
/// containing a `mesh_gateway_wire::AggregatedJobResult`.
pub async fn poll_user_decrypt_result(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
) -> Result<Option<UserDecryptAggregate>> {
    let (pylon_peer, result) = gateway.wait_job_result_resolved(job_id).await?;
    
    let payload = match result {
        ResolvedJobResult::Payload(p) => p,
        ResolvedJobResult::PublishedManifest { .. } => {
            return Err(anyhow!("decrypt expected payload result, got manifest"));
        }
    };
    let agg = decode_aggregated_result_from_peer(gateway, pylon_peer, payload).await?;
    
    if !matches!(agg.job_type, JobType::UserDecrypt) {
        return Err(anyhow!(
            "job type mismatch: expected UserDecrypt, got {:?}",
            agg.job_type
        ));
    }

    let mut responses = Vec::with_capacity(agg.proto_results.len());
    for entry in agg.proto_results {
        let resp = UserDecryptionResponse::decode(entry.bytes.as_slice())
            .map_err(|e| anyhow!("failed to decode UserDecryptionResponse: {e}"))?;
        responses.push(resp);
    }

    Ok(Some(UserDecryptAggregate {
        job_id: agg.job_id,
        responses,
    }))
}

/// Convenience helper for creating a public decryption request payload.
///
/// This does not try to replicate the full internal KMS client request-building
/// semantics; it only builds the protobuf message shell. If you need the exact
/// `kms_core_client` request creation flow with domain/external handle logic,
/// build the request using the existing KMS client code and pass it to
/// `submit_public_decrypt(...)`.
pub fn make_public_decryption_request(
    request_id_hex: String,
    ciphertexts: Vec<TypedCiphertext>,
    key_id_hex: String,
    domain: Option<kms_api::kms::v1::Eip712DomainMsg>,
    extra_data: Vec<u8>,
) -> PublicDecryptionRequest {
    PublicDecryptionRequest {
        request_id: Some(kms_api::kms::v1::RequestId {
            request_id: request_id_hex,
        }),
        ciphertexts,
        key_id: Some(kms_api::kms::v1::RequestId {
            request_id: key_id_hex,
        }),
        domain,
        extra_data,
        context_id: None,
        epoch_id: None,
    }
}

/// Convenience helper for creating a user decryption request payload.
///
/// As with `make_public_decryption_request`, this is a protobuf message builder,
/// not a replacement for the richer request-generation logic in the existing KMS client.
#[allow(clippy::too_many_arguments)]
pub fn make_user_decryption_request(
    request_id_hex: String,
    typed_ciphertexts: Vec<TypedCiphertext>,
    key_id_hex: String,
    client_address: String,
    enc_key: Vec<u8>,
    domain: kms_api::kms::v1::Eip712DomainMsg,
    extra_data: Vec<u8>,
) -> UserDecryptionRequest {
    UserDecryptionRequest {
        request_id: Some(kms_api::kms::v1::RequestId {
            request_id: request_id_hex,
        }),
        typed_ciphertexts,
        key_id: Some(kms_api::kms::v1::RequestId {
            request_id: key_id_hex,
        }),
        client_address,
        enc_key,
        domain: Some(domain),
        extra_data,
        context_id: None,
        epoch_id: None,
    }
}

async fn decode_aggregated_result(
    gateway: &mut MeshGatewayClient,
    payload: Payload,
) -> Result<AggregatedJobResult> {
    let bytes = match payload {
        Payload::Inline(b) => b,
        Payload::BlobRef(blob_id) => {
            let pylon = gateway.choose_pylon_public().await?;
            gateway
                .raw_client_mut()
                .get_blob(pylon, blob_id)
                .await
                .map_err(|e| anyhow!("failed to fetch aggregated result blob: {e}"))?
        }
    };

    mesh_gateway_wire::decode::<AggregatedJobResult>(&bytes)
        .map_err(|e| anyhow!("failed to decode AggregatedJobResult: {e}"))
}

async fn decode_aggregated_result_from_peer(
    gateway: &mut MeshGatewayClient,
    pylon_peer: PeerId,
    payload: Payload,
) -> Result<mesh_gateway_wire::AggregatedJobResult> {
    let bytes = match payload {
        Payload::Inline(b) => b,
        Payload::BlobRef(blob_id) => gateway.get_blob_from_pylon(pylon_peer, blob_id).await?,
    };

    mesh_gateway_wire::decode::<mesh_gateway_wire::AggregatedJobResult>(&bytes)
        .map_err(|e| anyhow!("failed to decode AggregatedJobResult: {e}"))
}

fn expect_aggregated_payload(result: JobResultPayload) -> Result<Payload> {
    match result {
        JobResultPayload::Payload(payload) => Ok(payload),
        JobResultPayload::PublishedManifest { manifest_cid } => Err(anyhow!(
            "expected aggregated decrypt result payload, got published manifest result: {:?}",
            manifest_cid
        )),
    }
}