use anyhow::{anyhow, Result};
use alloy_primitives::Signature;
use alloy_sol_types::{eip712_domain, Eip712Domain};
use kms_api::kms::v1::{
    PublicDecryptionRequest, PublicDecryptionResponse, TypedPlaintext, UserDecryptionRequest,
    //UserDecryptionResponse,
};
use kms_api::rpc_types::protobuf_to_alloy_domain;
use kms_lib::client::{
    client_wasm::Client,
    user_decryption_wasm::ParsedUserDecryptionRequest,
};
use kms_lib::cryptography::internal_crypto_types::{ UnifiedPrivateEncKey, UnifiedPublicEncKey };

use kms_lib::engine::base::compute_pt_message_hash;

use crate::decrypt::{PublicDecryptAggregate, UserDecryptAggregate};

/// A dummy EIP-712 domain matching the current `kms_core_client` fallback behavior.
///
/// This is only used when verifying a public decrypt aggregate without the original
/// request object, mirroring the old core-client `dummy_domain()` path.
fn dummy_domain() -> Eip712Domain {
    eip712_domain!(
        name: "Authorization token",
        version: "1",
        chain_id: 8006,
        verifying_contract: alloy_primitives::address!("66f9664f97F2b50F62D13eA064982f936dE76657"),
    )
}

fn dummy_handle() -> Vec<u8> {
    vec![23_u8; 32]
}

fn recover_address_from_pt_signature(
    external_sig: &[u8],
    plaintexts: &[TypedPlaintext],
    external_handles: Vec<Vec<u8>>,
    domain: Eip712Domain,
    extra_data: Vec<u8>,
) -> Result<alloy_primitives::Address> {
    if external_sig.len() != 65 {
        return Err(anyhow!(
            "expected external signature of length 65 bytes, got {}",
            external_sig.len()
        ));
    }

    let sig = Signature::from_bytes_and_parity(external_sig, external_sig[64] & 0x01 == 0);
    let hash = compute_pt_message_hash(external_handles, &plaintexts.to_vec(), domain, extra_data)?;
    let addr = sig.recover_address_from_prehash(&hash)?;
    Ok(addr)
}

fn check_external_decryption_signatures(
    responses: &[PublicDecryptionResponse],
    expected_answer: &TypedPlaintext,
    external_handles: &[Vec<u8>],
    domain: &Eip712Domain,
    kms_addrs: &[alloy_primitives::Address],
) -> Result<()> {
    let mut results = Vec::new();

    for response in responses {
        let payload = response
            .payload
            .as_ref()
            .ok_or_else(|| anyhow!("missing payload in PublicDecryptionResponse"))?;

        let addr = recover_address_from_pt_signature(
            &response.external_signature,
            &payload.plaintexts,
            external_handles.to_vec(),
            domain.clone(),
            response.extra_data.clone(),
        )?;

        if !kms_addrs.contains(&addr) {
            return Err(anyhow!(
                "external plaintext signature recovered unauthorized address {addr}"
            ));
        }

        for pt in &payload.plaintexts {
            results.push(pt.clone());
        }
    }

    for pt in results {
        if pt != *expected_answer {
            return Err(anyhow!(
                "public decrypt plaintext mismatch: expected {:?}, got {:?}",
                expected_answer,
                pt
            ));
        }
    }

    Ok(())
}

/// Verify a decoded public decrypt aggregate.
///
/// This mirrors the verification shape from `kms_core_client`:
/// - internal signature / response consistency through `process_decryption_resp`
/// - external EIP-712 plaintext signature verification
/// - optional plaintext equality check
///
/// `dec_req`:
/// - pass `Some(&request)` for full verification using the original request domain
///   and external handles
/// - pass `None` only for the fallback/testing mode that mimics old core-client
///
/// `expected_answer`:
/// - if `Some`, this plaintext is enforced against all returned plaintexts
/// - if `None`, the first returned plaintext becomes the reference
pub fn verify_public_decrypt_aggregate(
    internal_client: &Client,
    dec_req: Option<&PublicDecryptionRequest>,
    aggregate: &PublicDecryptAggregate,
    expected_answer: Option<TypedPlaintext>,
    kms_addrs: &[alloy_primitives::Address],
) -> Result<Vec<PublicDecryptionResponse>> {
    let responses = aggregate.responses.clone();

    if responses.is_empty() {
        return Err(anyhow!("public decrypt aggregate is empty"));
    }

    let num_expected = responses.len() as u32;

    // Internal verification first.
    internal_client.process_decryption_resp(dec_req.cloned(), &responses, num_expected)?;

    let (domain, external_handles) = if let Some(req) = dec_req {
        let domain_msg = req
            .domain
            .as_ref()
            .ok_or_else(|| anyhow!("public decrypt request missing domain"))?;
        let domain = protobuf_to_alloy_domain(domain_msg)?;
        let handles = req
            .ciphertexts
            .iter()
            .map(|ct| ct.external_handle.clone())
            .collect::<Vec<_>>();
        (domain, handles)
    } else {
        let num_handles = responses[0]
            .payload
            .as_ref()
            .ok_or_else(|| anyhow!("missing payload in first PublicDecryptionResponse"))?
            .plaintexts
            .len();

        (dummy_domain(), vec![dummy_handle(); num_handles])
    };

    let expected = if let Some(pt) = expected_answer {
        pt
    } else {
        responses[0]
            .payload
            .as_ref()
            .ok_or_else(|| anyhow!("missing payload in first PublicDecryptionResponse"))?
            .plaintexts
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("missing plaintext in first PublicDecryptionResponse"))?
    };

    check_external_decryption_signatures(
        &responses,
        &expected,
        &external_handles,
        &domain,
        kms_addrs,
    )?;

    Ok(responses)
}

/// Context object for reconstructing a user decrypt aggregate.
///
/// This intentionally carries the exact inputs that the existing KMS-side
/// reconstruction path needs:
/// - original protobuf request
/// - parsed request
/// - encapsulation public key
/// - encapsulation secret key
///
/// The concrete `EncPk` / `EncSk` types depend on your current `kms_lib` public API.
/// Wire them from the return value of your Mesh-side `user_decryption_request(...)` builder.
///
/// This avoids inventing fake types here while still giving you a stable Mesh-side
/// function boundary.
pub struct UserDecryptReconstructionContext {
    pub request: UserDecryptionRequest,
    pub parsed_request: ParsedUserDecryptionRequest,
    pub enc_pk: UnifiedPublicEncKey,
    pub enc_sk: UnifiedPrivateEncKey,
}

/// Reconstruct a user decrypt aggregate using the existing `kms_lib` logic.
///
/// This is the Mesh-side equivalent of the old core-client reconstruction step.
///
/// You provide the exact context produced when the original user decrypt request
/// was created.
pub fn reconstruct_user_decrypt_aggregate(
    internal_client: &Client,
    aggregate: &UserDecryptAggregate,
    ctx: &UserDecryptReconstructionContext,
) -> Result<Vec<TypedPlaintext>> {

    let domain_msg = ctx
        .request
        .domain
        .as_ref()
        .ok_or_else(|| anyhow!("user decrypt request missing domain"))?;
    let eip712_domain = protobuf_to_alloy_domain(domain_msg)?;

    internal_client
        .process_user_decryption_resp(
            &ctx.parsed_request,
            &eip712_domain,
            &aggregate.responses,
            &ctx.enc_pk,
            &ctx.enc_sk,
        )
        .map_err(|e| anyhow!("user decrypt reconstruction failed: {e}"))
}