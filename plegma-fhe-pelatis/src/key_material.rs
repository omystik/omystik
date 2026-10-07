use anyhow::{anyhow, Result};
use alloy_primitives::Signature;
use alloy_sol_types::Eip712Domain;
use prost::Message;
use libp2p::PeerId;
use tfhe::zk::CompactPkeCrs;
use tfhe::{CompactPublicKey, ServerKey};

use kms_api::kms::v1::{
    CrsGenRequest, CrsGenResult, Eip712DomainMsg, FheParameter, KeyGenPreprocRequest, // KeyDigest
    KeyGenPreprocResult, KeyGenRequest, KeyGenResult, KeySetAddedInfo, KeySetConfig,
};
// use kms_api::rpc_types::protobuf_to_alloy_domain;
use kms_api::solidity_types::{CrsgenVerification, KeygenVerification};
use kms_api::RequestId; // {KeyId, 

use kms_lib::engine::base::{
    hash_sol_struct, safe_serialize_hash_element_versioned, DSEP_PUBDATA_CRS, DSEP_PUBDATA_KEY,
};
use kms_lib::util::key_setup::test_tools::{load_material_from_storage, load_pk_from_storage};

use mesh_gateway_wire::{AggregatedJobResult, BlobId, JobId, JobMetadata, JobResultPayload, JobType, Payload};

use crate::pylon_client::{MeshGatewayClient, ResolvedJobResult};
use crate::materialize::{materialize_crs_bundle, materialize_key_bundle};

#[derive(Debug, Clone)]
pub struct KeyGenPreprocAggregate {
    pub job_id: JobId,
    pub responses: Vec<KeyGenPreprocResult>,
}

#[derive(Debug, Clone)]
pub struct KeyGenAggregate {
    pub job_id: JobId,
    pub responses: Vec<KeyGenResult>,
}

#[derive(Debug, Clone)]
pub struct CrsGenAggregate {
    pub job_id: JobId,
    pub responses: Vec<CrsGenResult>,
}

pub async fn submit_keygen_preproc(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
    request: KeyGenPreprocRequest,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(request.encode_to_vec());
    gateway
        .submit_job(job_id, JobType::KeygenPreproc, payload, metadata)
        .await
}

pub async fn submit_keygen(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
    request: KeyGenRequest,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(request.encode_to_vec());
    gateway.submit_job(job_id, JobType::Keygen, payload, metadata).await
}

pub async fn submit_crsgen(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
    request: CrsGenRequest,
    metadata: JobMetadata,
) -> Result<()> {
    let payload = Payload::Inline(request.encode_to_vec());
    gateway.submit_job(job_id, JobType::CrsGen, payload, metadata).await
}

pub async fn poll_keygen_preproc_result(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
) -> Result<Option<KeyGenPreprocAggregate>> {
    let (pylon_peer, result) = gateway.wait_job_result(job_id).await?;
    let payload = expect_aggregated_payload(result)?;

    let agg = decode_aggregated_result_from_peer(gateway, pylon_peer, payload).await?;

    if !matches!(agg.job_type, JobType::KeygenPreproc) {
        return Err(anyhow!(
            "job type mismatch: expected KeygenPreproc, got {:?}",
            agg.job_type
        ));
    }

    let mut responses = Vec::with_capacity(agg.proto_results.len());
    for entry in agg.proto_results {
        responses.push(
            KeyGenPreprocResult::decode(entry.bytes.as_slice())
                .map_err(|e| anyhow!("failed to decode KeyGenPreprocResult: {e}"))?,
        );
    }

    Ok(Some(KeyGenPreprocAggregate {
        job_id: agg.job_id,
        responses,
    }))
}

pub async fn poll_keygen_result(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
) -> Result<Option<BlobId>> {
    let (_pylon_peer, result) = gateway.wait_job_result_resolved(job_id).await?;

    let manifest_cid = match result {
        ResolvedJobResult::PublishedManifest { manifest_cid } => manifest_cid,
        ResolvedJobResult::Payload(_) => {
            return Err(anyhow!("expected manifest result for keygen/crsgen"));
        }
    };

    Ok(Some(manifest_cid))
}

pub async fn poll_crsgen_result(
    gateway: &mut MeshGatewayClient,
    job_id: JobId,
) -> Result<Option<BlobId>> {
    let (_pylon_peer, result) = gateway.wait_job_result_resolved(job_id).await?;

    let manifest_cid = match result {
        ResolvedJobResult::PublishedManifest { manifest_cid } => manifest_cid,
        ResolvedJobResult::Payload(_) => {
            return Err(anyhow!("expected manifest result for keygen/crsgen"));
        }
    };

    Ok(Some(manifest_cid))
}

pub async fn materialize_and_verify_keygen(
    gateway: &mut MeshGatewayClient,
    cache_root: &std::path::Path,
    key_id: RequestId,
    preproc_id: RequestId,
    domain: &Eip712Domain,
    responses: &[KeyGenResult],
    kms_addrs: &[alloy_primitives::Address],
    party_id_for_load: usize,
) -> Result<()> {
    materialize_key_bundle(gateway, key_id.into_bytes()).await?;

    let public_key: CompactPublicKey =
        load_pk_from_storage(Some(cache_root), &key_id, party_id_for_load).await;

    let server_key: ServerKey = load_material_from_storage(
        Some(cache_root),
        &key_id,
        kms_api::rpc_types::PubDataType::ServerKey,
        party_id_for_load,
    )
    .await;

    for response in responses {
        let resp_req_id: RequestId = response
            .request_id
            .clone()
            .ok_or_else(|| anyhow!("missing request_id in KeyGenResult"))?
            .try_into()?;

        if resp_req_id != key_id {
            return Err(anyhow!(
                "keygen response request_id mismatch: expected {}, got {}",
                key_id,
                resp_req_id
            ));
        }

        check_standard_keyset_ext_signature(
            &public_key,
            &server_key,
            &preproc_id,
            &key_id,
            &response.external_signature,
            domain,
            kms_addrs,
        )?;
    }

    Ok(())
}

pub async fn materialize_and_verify_crsgen(
    gateway: &mut MeshGatewayClient,
    cache_root: &std::path::Path,
    crs_id: RequestId,
    domain: &Eip712Domain,
    responses: &[CrsGenResult],
    kms_addrs: &[alloy_primitives::Address],
    party_id_for_load: usize,
) -> Result<()> {
    materialize_crs_bundle(gateway, crs_id.into_bytes()).await?;

    let crs: CompactPkeCrs = load_material_from_storage(
        Some(cache_root),
        &crs_id,
        kms_api::rpc_types::PubDataType::CRS,
        party_id_for_load,
    )
    .await;

    for response in responses {
        let resp_req_id: RequestId = response
            .request_id
            .clone()
            .ok_or_else(|| anyhow!("missing request_id in CrsGenResult"))?
            .try_into()?;

        if resp_req_id != crs_id {
            return Err(anyhow!(
                "crsgen response request_id mismatch: expected {}, got {}",
                crs_id,
                resp_req_id
            ));
        }

        check_crsgen_ext_signature(
            &crs,
            &crs_id,
            &response.external_signature,
            domain,
            kms_addrs,
        )?;
    }

    Ok(())
}

pub fn make_keygen_preproc_request(
    request_id: RequestId,
    params: FheParameter,
    keyset_config: Option<KeySetConfig>,
    domain: Option<Eip712DomainMsg>,
) -> KeyGenPreprocRequest {
    KeyGenPreprocRequest {
        request_id: Some(request_id.into()),
        params: params as i32,
        keyset_config,
        domain,
        context_id: None,
        epoch_id: None,
    }
}

pub fn make_keygen_request(
    request_id: RequestId,
    preproc_id: RequestId,
    params: Option<FheParameter>,
    domain: Option<Eip712DomainMsg>,
    keyset_config: Option<KeySetConfig>,
    keyset_added_info: Option<KeySetAddedInfo>,
) -> KeyGenRequest {
    KeyGenRequest {
        request_id: Some(request_id.into()),
        params: params.map(|p| p as i32), // params: params.map(|p| p as i32),
        preproc_id: Some(preproc_id.into()),
        domain,
        keyset_config,
        keyset_added_info,
        context_id: None,
        epoch_id: None,
    }
}

pub fn make_crsgen_request(
    request_id: RequestId,
    params: FheParameter,
    max_num_bits: Option<u32>,
    domain: Option<Eip712DomainMsg>,
) -> CrsGenRequest {
    CrsGenRequest {
        request_id: Some(request_id.into()),
        params: params as i32,
        max_num_bits,
        domain,
        context_id: None,
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

fn recover_address_from_ext_signature<S: alloy_sol_types::SolStruct>(
    data: &S,
    domain: &Eip712Domain,
    external_sig: &[u8],
) -> Result<alloy_primitives::Address> {
    if external_sig.len() != 65 {
        return Err(anyhow!(
            "expected external signature length 65, got {}",
            external_sig.len()
        ));
    }

    let sig = Signature::from_bytes_and_parity(external_sig, external_sig[64] & 0x01 == 0);
    let hash = hash_sol_struct(data, domain)?;
    Ok(sig.recover_address_from_prehash(&hash)?)
}

fn check_standard_keyset_ext_signature(
    public_key: &CompactPublicKey,
    server_key: &ServerKey,
    prep_id: &RequestId,
    key_id: &RequestId,
    external_sig: &[u8],
    domain: &Eip712Domain,
    kms_addrs: &[alloy_primitives::Address],
) -> Result<()> {
    let server_key_digest = safe_serialize_hash_element_versioned(&DSEP_PUBDATA_KEY, server_key)?;
    let public_key_digest = safe_serialize_hash_element_versioned(&DSEP_PUBDATA_KEY, public_key)?;

    let sol_type = KeygenVerification::new(prep_id, key_id, server_key_digest, public_key_digest);
    let addr = recover_address_from_ext_signature(&sol_type, domain, external_sig)?;

    if kms_addrs.contains(&addr) {
        Ok(())
    } else {
        Err(anyhow!(
            "external signature verification failed for keygen: unauthorized signer {addr}"
        ))
    }
}

fn check_crsgen_ext_signature(
    crs: &CompactPkeCrs,
    crs_id: &RequestId,
    external_sig: &[u8],
    domain: &Eip712Domain,
    kms_addrs: &[alloy_primitives::Address],
) -> Result<()> {
    let crs_digest = safe_serialize_hash_element_versioned(&DSEP_PUBDATA_CRS, crs)?;
    let max_num_bits = threshold_fhe::execution::zk::ceremony::max_num_bits_from_crs(crs);

    let sol_type = CrsgenVerification::new(crs_id, max_num_bits, crs_digest);
    let addr = recover_address_from_ext_signature(&sol_type, domain, external_sig)?;

    if kms_addrs.contains(&addr) {
        Ok(())
    } else {
        Err(anyhow!(
            "external signature verification failed for crsgen: unauthorized signer {addr}"
        ))
    }
}

fn expect_aggregated_payload(result: JobResultPayload) -> Result<Payload> {
    match result {
        JobResultPayload::Payload(payload) => Ok(payload),
        JobResultPayload::PublishedManifest { manifest_cid } => Err(anyhow!(
            "expected aggregated result payload, got published manifest cid: {:?}",
            manifest_cid
        )),
    }
}

fn expect_published_manifest(result: JobResultPayload) -> Result<[u8; 32]> {
    match result {
        JobResultPayload::PublishedManifest { manifest_cid } => Ok(manifest_cid),
        JobResultPayload::Payload(_) => Err(anyhow!(
            "expected published manifest job result, got generic payload"
        )),
    }
}