use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::{
    task::JoinSet,
    time::{error::Elapsed, timeout_at},
};
use tracing::Instrument;

use crate::{
    algebra::structure_traits::Ring,
    error::error_handler::anyhow_error_and_log,
    execution::runtime::{
        party::Role,
        session::{BaseSessionHandles, LargeSessionHandles},
    },
    networking::p2p::NetworkMsgKind,
    networking::value::NetworkValue,
};

/// Helper function to check that senders and receivers make sense, returns [false] if they don't and adds a log.
/// Returns true if everything is fine.
/// By not making sense, we mean that the party is either the same as the currently executing party or that the
/// currently executing party is in conflict with the sender/receiver, or the sender/receiver is corrupt
fn check_roles<'a, L: LargeSessionHandles + 'a>(
    communicating_with: &Role,
    session: &L,
) -> anyhow::Result<bool> {
    // Ensure we don't send to ourself
    if communicating_with == &session.my_role() {
        tracing::info!("You are trying to communicate with yourself.");
        return Ok(false);
    }
    // Ensure we don't send to corrupt parties
    if session.corrupt_roles().contains(communicating_with) {
        tracing::warn!(
            "You are communicating with a corrupt party: {:?}",
            communicating_with
        );
        return Ok(false);
    }
    // Ensure we don't send to disputed parties
    // Observe that if a party is corrupt it will also be in dispute, hence we have already returned above and only write the log that they are corrupt
    if session
        .disputed_roles()
        .get(&session.my_role())
        .contains(communicating_with)
    {
        tracing::info!(
            "You are communicating with a disputed party: {:?}",
            communicating_with
        );
        return Ok(false);
    }
    Ok(true)
}

fn check_talking_to_myself<'a, B: BaseSessionHandles + 'a>(
    r: &Role,
    session: &B,
) -> anyhow::Result<bool> {
    Ok(r != &session.my_role())
}

/// Send specific values to all parties.
/// Each party is supposed to receive a specific value, mapped to their role in `values_to_send`.
/// Automatically increases the round counter when called
/// Note: This also sends to corrupt parties
pub async fn send_to_parties<'a, Z: Ring, B: BaseSessionHandles + 'a>(
    values_to_send: &HashMap<Role, NetworkValue<Z>>,
    session: &B,
) -> anyhow::Result<()> {
    session.network().increase_round_counter().await;
    // pass the always-true fn as check-fn, since we're checking for equal sender and receiver inside internal_send_to_parties
    internal_send_to_parties(values_to_send, session, &|_a: &Role, _b: &B| Ok(true)).await?;
    Ok(())
}

/// Send specific values to specific parties, while validating that the parties are sensible within the session.
/// I.e. not the sending party or in dispute or corrupt.
/// Each party is supposed to receive a specific value, mapped to their role in `values_to_send`.
/// Automatically increases the round counter when called
pub async fn send_to_honest_parties<'a, Z: Ring, L: LargeSessionHandles + 'a>(
    values_to_send: &HashMap<Role, NetworkValue<Z>>,
    session: &'a L,
) -> anyhow::Result<()> {
    session.network().increase_round_counter().await;
    internal_send_to_parties(values_to_send, session, &check_roles).await?;
    Ok(())
}

/// Add a job of sending specific values to specific parties.
/// Each party is supposed to receive a specific value, mapped to their role in `values_to_send`.
async fn internal_send_to_parties<'a, Z: Ring, B: BaseSessionHandles + 'a>(
    values_to_send: &HashMap<Role, NetworkValue<Z>>,
    session: &'a B,
    check_fn: &'a (dyn Fn(&Role, &'a B) -> anyhow::Result<bool> + Sync + Send + 'a),
) -> anyhow::Result<()> {
    let my_role = session.my_role();
    for (cur_receiver, cur_value) in values_to_send.iter() {
        // do not send to myself
        if cur_receiver != &my_role {
            // Ensure the party we want to send to passes the check we specified
            if check_fn(cur_receiver, session)? {
                // Choose lane from the message itself.
                // tag.kind MUST match this, since routing is keyed by (sender, kind).
                let kind = cur_value.msg_kind();
                let value_to_send = Arc::new(cur_value.to_network());

                session
                    .network()
                    .send_kind(value_to_send, cur_receiver, kind)
                    .await?;
            } else {
                tracing::warn!(
                    "I am {:?} trying to send to receiver {:?}, who doesn't pass check",
                    my_role,
                    cur_receiver
                );
                continue;
            }
        }
    }
    Ok(())
}

/// Receive specific values from specific parties.
///
/// Legacy/default behavior: receives on lane `Other`.
/// Protocols that use lane-aware routing should call `receive_from_parties_kind`.
pub async fn receive_from_parties<'a, Z: Ring, S: BaseSessionHandles + 'a>(
    senders: &HashSet<Role>,
    session: &S,
) -> anyhow::Result<HashMap<Role, NetworkValue<Z>>> {
    receive_from_parties_kind(senders, session, NetworkMsgKind::Other).await
}

/// Receive specific values from specific parties, excluding disputed/corrupt parties.
///
/// Legacy/default behavior: receives on lane `Other`.
/// Protocols that use lane-aware routing should call `receive_from_parties_w_dispute_kind`.
pub async fn receive_from_parties_w_dispute<'a, Z: Ring, L: LargeSessionHandles + 'a>(
    senders: &HashSet<Role>,
    session: &L,
) -> anyhow::Result<HashMap<Role, NetworkValue<Z>>> {
    receive_from_parties_w_dispute_kind(senders, session, NetworkMsgKind::Other).await
}

/// Receive specific values from specific parties on a given lane (NetworkMsgKind).
/// Returns [`NetworkValue::Bot`] in case of failure to receive but without adding parties to the corruption or dispute sets.
pub async fn receive_from_parties_kind<'a, Z: Ring, S: BaseSessionHandles + 'a>(
    senders: &HashSet<Role>,
    session: &S,
    kind: NetworkMsgKind,
) -> anyhow::Result<HashMap<Role, NetworkValue<Z>>> {
    let mut receive_job = JoinSet::new();
    internal_receive_from_parties_kind(
        &mut receive_job,
        senders,
        session,
        kind,
        &check_talking_to_myself,
    )
    .await?;

    let mut res = HashMap::with_capacity(senders.len());
    while let Some(received_data) = receive_job.join_next().await {
        let (sender_role, sender_data) = received_data?;
        let _ = res.insert(sender_role, sender_data);
    }
    Ok(res)
}

/// Receive specific values from specific parties on a given lane (NetworkMsgKind),
/// excluding disputed/corrupt parties.
/// Returns [`NetworkValue::Bot`] in case of failure to receive but without adding parties to the corruption or dispute sets.
pub async fn receive_from_parties_w_dispute_kind<'a, Z: Ring, L: LargeSessionHandles + 'a>(
    senders: &HashSet<Role>,
    session: &L,
    kind: NetworkMsgKind,
) -> anyhow::Result<HashMap<Role, NetworkValue<Z>>> {
    let mut receive_job = JoinSet::new();
    internal_receive_from_parties_kind(&mut receive_job, senders, session, kind, &check_roles)
        .await?;

    let mut res = HashMap::with_capacity(senders.len());
    while let Some(received_data) = receive_job.join_next().await {
        let (sender_role, sender_data) = received_data?;
        let _ = res.insert(sender_role, sender_data);
    }
    Ok(res)
}

/// Add a job of receiving values from specific parties on a given lane.
/// Each of the senders are contained in [senders].
/// If we don't receive anything, the value [NetworkValue::Bot] is returned.
async fn internal_receive_from_parties_kind<'a, Z: Ring, B: BaseSessionHandles + 'a>(
    jobs: &mut JoinSet<(Role, NetworkValue<Z>)>,
    senders: &HashSet<Role>,
    session: &'a B,
    kind: NetworkMsgKind,
    check_fn: &'a (dyn Fn(&Role, &'a B) -> anyhow::Result<bool> + Sync + Send + 'a),
) -> anyhow::Result<()> {
    let deserialization_runtime = session.get_deserialization_runtime();

    for cur_sender in senders {
        // Do not even try to receive from myself
        if cur_sender == &session.my_role() {
            continue;
        }

        // Ensure we want to receive from that sender (e.g. not from ourself or a malicious party)
        if check_fn(cur_sender, session)? {
            let networking = Arc::clone(session.network());
            let role_to_receive_from = *cur_sender;
            let deadline = session.network().get_timeout_current_round().await;

            jobs.spawn(async move {
                // CRITICAL: lane-aware receive. Do NOT call networking.receive() here.
                let received = timeout_at(
                    deadline,
                    networking.receive_kind(&role_to_receive_from, kind),
                )
                .await
                .unwrap_or_else(|e| {
                    Err(anyhow_error_and_log(format!(
                        "Timed out with deadline {deadline:?} from {role_to_receive_from:?} (kind={kind:?}) : {e:?}"
                    )))
                });

                match NetworkValue::<Z>::from_network(received, deserialization_runtime).await {
                    Ok(val) => (role_to_receive_from, val),
                    // We got an unexpected type of value from the network.
                    _ => (role_to_receive_from, NetworkValue::Bot),
                }
            });
        } else {
            tracing::info!(
                "I am {:?} trying to receive from sender {:?}, who doesn't pass check",
                session.my_role(),
                cur_sender
            );
        }
    }

    Ok(())
}

/// Send to all parties and automatically increase round counter
pub async fn send_to_all<T, Z: Ring, B: BaseSessionHandles>(
    session: &B,
    sender: &Role,
    msg: T,
) -> anyhow::Result<()>
where
    T: AsRef<NetworkValue<Z>>,
{
    // Choose lane from the value (protocol-level truth)
    let msg_ref: &NetworkValue<Z> = msg.as_ref();
    let kind = msg_ref.msg_kind();
    let serialized_message = Arc::new(msg_ref.to_network());

    // Preserve existing semantics: one round tick per broadcast send_to_all() call.
    session.network().increase_round_counter().await;

    // Send the exact same bytes to all peers (no per-peer serialization).
    for other_role in session.roles() {
        if other_role == sender {
            continue;
        }
        // Use kind-aware send so tag.kind == msg_kind().
        session
            .network()
            .send_kind(Arc::clone(&serialized_message), other_role, kind)
            .await?;
    }

    Ok(())
}

/// Spawns receive tasks and matches the incoming messages according to the match_network_value_fn.
///
/// **NOTE: We do not try to receive any value from the non_answering_parties set.**
pub async fn generic_receive_from_all_senders<V, Z: Ring, B: BaseSessionHandles>(
    jobs: &mut JoinSet<Result<(Role, anyhow::Result<V>), Elapsed>>,
    session: &B,
    receiver: &Role,
    senders: &HashSet<Role>,
    non_answering_parties: Option<&HashSet<Role>>,
    match_network_value_fn: fn(network_value: NetworkValue<Z>, id: &Role) -> anyhow::Result<V>,
) where
    V: Send + 'static,
{
    generic_receive_from_all_senders_kind(
        jobs,
        session,
        receiver,
        senders,
        non_answering_parties,
        NetworkMsgKind::Other,
        match_network_value_fn,
    )
    .await
}

pub async fn generic_receive_from_all<V, Z: Ring, B: BaseSessionHandles>(
    jobs: &mut JoinSet<Result<(Role, anyhow::Result<V>), Elapsed>>,
    session: &B,
    receiver: &Role,
    non_answering_parties: Option<&HashSet<Role>>,
    match_network_value_fn: fn(network_value: NetworkValue<Z>, id: &Role) -> anyhow::Result<V>,
) where
    V: Send + 'static,
{
    generic_receive_from_all_kind(
        jobs,
        session,
        receiver,
        non_answering_parties,
        NetworkMsgKind::Other,
        match_network_value_fn,
    )
    .await
}

pub async fn generic_receive_from_all_senders_kind<V, Z: Ring, B: BaseSessionHandles>(
    jobs: &mut JoinSet<Result<(Role, anyhow::Result<V>), Elapsed>>,
    session: &B,
    receiver: &Role,
    senders: &HashSet<Role>,
    non_answering_parties: Option<&HashSet<Role>>,
    kind: NetworkMsgKind,
    match_network_value_fn: fn(network_value: NetworkValue<Z>, id: &Role) -> anyhow::Result<V>,
) where
    V: Send + 'static,
{
    let deserialization_runtime = session.get_deserialization_runtime();
    let binding = HashSet::new();
    let non_answering_parties = non_answering_parties.unwrap_or(&binding);

    // IMPORTANT: copy receiver role into owned value so the spawned task is 'static-safe
    let receiver_role: Role = *receiver;

    for sender in senders {
        if non_answering_parties.contains(sender) || receiver_role == *sender {
            continue;
        }

        let sender: Role = *sender;
        let networking = Arc::clone(session.network());
        let timeout_deadline = session.network().get_timeout_current_round().await;

        let task = async move {
            let stripped_message =
                timeout_at(timeout_deadline, networking.receive_kind(&sender, kind)).await;

            match stripped_message {
                Ok(stripped_message) => {
                    let parsed = match NetworkValue::<Z>::from_network(
                        stripped_message,
                        deserialization_runtime,
                    )
                    .await
                    {
                        // CRITICAL: pass the SENDER role, not the receiver role.
                        Ok(x) => match_network_value_fn(x, &sender),
                        Err(e) => Err(e),
                    };
                    Ok((sender, parsed))
                }
                Err(e) => {
                    tracing::warn!(
                        "Sender {sender} timed out when sending to {receiver_role} (kind={kind:?})"
                    );
                    Err(e)
                }
            }
        }
        .instrument(tracing::Span::current());

        jobs.spawn(task);
    }
}


pub async fn generic_receive_from_all_kind<V, Z: Ring, B: BaseSessionHandles>(
    jobs: &mut JoinSet<Result<(Role, anyhow::Result<V>), Elapsed>>,
    session: &B,
    receiver: &Role,
    non_answering_parties: Option<&HashSet<Role>>,
    kind: NetworkMsgKind,
    match_network_value_fn: fn(network_value: NetworkValue<Z>, id: &Role) -> anyhow::Result<V>,
) where
    V: Send + 'static,
{
    generic_receive_from_all_senders_kind(
        jobs,
        session,
        receiver,
        session.roles(),
        non_answering_parties,
        kind,
        match_network_value_fn,
    )
    .await
}
