# Getting Started

How to build ØMYSTIK and bring up a working local cluster.

>[!NOTE]
> We use 4 nodes (4 MPC parties) based on the $3t + 1$ formula, where $t$ is the max number of
> compromised nodes. 

> [!WARNING]
> Every command here builds with the `insecure` feature, which is in the workspace default
> feature set. This is a **development** configuration and produces binaries unsuitable for
> production. See [SECURITY.md](../SECURITY.md#insecure-is-a-default-feature).

---

## 1. Prerequisites

- **Rust**: recent stable toolchain, edition 2021
- **Build essentials**: a C toolchain, for the native code in the cryptographic
  dependencies
- **Disk and time**: the workspace is large and the FHE dependencies are heavy. Expect a
  long first build and several GB in `target/`.
- **[`cross`](https://github.com/cross-rs/cross)**: only if cross-compiling for edge
  hardware ([§8](#8))

```bash
cargo build --workspace
```

`kms/` is a **separate Cargo workspace** with its own members, and none of its crates are
members of the root one. `cargo locate-project --workspace` resolves to `kms/Cargo.toml`
from `kms/core/service`, and to the root manifest from `node_kryphos`.<br>
`--workspace` therefore builds ØMYSTIK's 24 crates, pulling in `kms/core/api`,
`kms/core/service`, `kms/core/grpc`, `kms/core/threshold`, `kms/core/libp2p`,
`kms/observability` and `kms/bc2wrap` as ordinary path dependencies. The two workspaces do
not conflict.

To build just one part, name it rather than the workspace:

```bash
cargo build -p node_kryphos --bin node_fhe --features "p2p insecure"
```

Because `[patch.crates-io]` points six TLS and attestation crates at git branches, the
first build needs network access to those repositories. See
[SECURITY.md](../SECURITY.md#cryptographic-dependencies-use-forks) for why that matters.

## 2. KMS configuration: Server (Kryphos' face A) and Client

- mpc_party face A ([kms/core/service/config](../kms/core/service/config/default_1.toml))
```
.
├── default_1.toml
├── default_2.toml
├── default_3.toml
├── default_4.toml
```
- actual kentr admin ([node_kentr/config](../node_kentr/config))
```
node_kentr/config
.
├── client_local_threshold.toml
└── client_remote_threshold.toml

```
- actual kentr_non-admin ([osatcon/headquarter/config](../osatcon/headquarter/config))
```
.
├── client_local_threshold.toml
└── client_remote_threshold.toml
```

## 3. Nodes Cluster configuration

Every node reads a TOML cluster file from `meshes_config/` and selects its own entry by
`--<role>-id`. Three variants exist per cluster:

| Variant | Use |
|---|---|
| `*_local.toml` | Everything on `127.0.0.1`, start here |
| `*_test.toml` | Mixed local and remote |
| `*.toml` | Production addresses |

```
mesh_a_cluster_local.toml     Mesh A: syndesmos + autonomos
mesh_c_cluster_local.toml     Mesh C: ypolo executors
mpc_cluster_local.toml        Mesh B: kryphos MPC parties' face B
gateway_cluster_local.toml    Pylon gateways (Face A + Face B)
kentr_cluster_local.toml      Kentr control nodes
```

> [!CAUTION]
> Committed configs assign each node a one-byte `secret_seed` from which its keypair is
> derived. Treat every peer identity in these files as public. Never reuse them outside a
> local test. See [SECURITY.md](../SECURITY.md#deterministic-node-identities-from-short-seeds).

### The Pylon's rendezvous addresses are in Rust, not in TOML

This one will cost you an afternoon if you meet it by accident.

The gateway configuration has a `rendezvous` field listing the Syndesmos nodes the Pylon
should register with. **None of the three shipped `gateway_cluster*.toml` files sets it.**
The field is declared with a serde default:

```rust
// meshes_config/src/gateway_config.rs
#[serde(default = "default_rendezvous_bootstraps_de")]
pub rendezvous: Vec<RendezvousABootstrap>,
```

so when the TOML omits it, the Pylon falls back to the values hardcoded in
`default_rendezvous_bootstraps()` in that same file. In every shipped configuration, that
function is therefore the only place the Pylon learns where Syndesmos is.

Those hardcoded values must match the `[[syndesmos]]` entries of the Mesh A cluster file you
are actually running. Out of the box they mirror `mesh_a_cluster_local.toml`:

| | `gateway_config.rs` default | `mesh_a_cluster_local.toml` |
|---|---|---|
| Syndesmos 1 | `/ip4/127.0.0.1/udp/9008/quic-v1` | `[[syndesmos]]` id 1, same address and peer id |
| Syndesmos 2 | `/ip4/127.0.0.1/udp/9002/quic-v1` | `[[syndesmos]]` id 2, same address and peer id |

So a purely local run works unchanged. **Switching to `_test` or production addresses does
not.** Change the Syndesmos address in the cluster TOML and the Pylon will keep dialling
`127.0.0.1`, register with nothing, and the mesh will look healthy while no node ever finds
the gateway.

Two ways out, either is fine:

- **Set `rendezvous` in your gateway TOML.** The field exists; filling it makes the TOML
  authoritative and the Rust default irrelevant. This is the better habit.
- **Edit `default_rendezvous_bootstraps()`** in `meshes_config/src/gateway_config.rs` to
  match, and rebuild. `mesh_a_cluster_local.toml` carries a reminder comment on the
  `listen_addr` line for exactly this reason.

Both the address and the peer id have to agree. A matching address with a stale peer id fails
the same way, and more confusingly, because the connection succeeds and the identity check
does not.

This hardcode will disappear when the mesh of gateway nodes (mesh-p) will be developed.

## 4. Storage environment

A storage node locates its data through environment variables. Set them **per shell**,
before launching a node. Each node needs its own tree
(see [NODE_LAYOUT.md](NODE_LAYOUT.md)):

```bash
export STORAGE_DIR="./nodeA/content/"
export STORAGE_MIDCID="./nodeA/metadata/midcid/"
export STORAGE_MERKLE="./nodeA/metadata/metamerkle/"
export STORAGE_METADATA="./nodeA/metadata/metamerkle/"
export BLOB_STORAGE_DIR="./nodeA/blob/"
```

To clear them:

```bash
unset STORAGE_DIR STORAGE_MIDCID STORAGE_MERKLE STORAGE_METADATA BLOB_STORAGE_DIR
```

## 5. Mesh A only: encrypted storage

The smallest useful setup: a rendezvous node and a peer exchanging encrypted files. No FHE
involved.

**Terminal 1, rendezvous (`syndesmos`)**

```bash
cd node_syndesmos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --syndesmos-id 1
```
if needed
```RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug```

**Terminal 2, peer (`autonomos`)**, with its own storage environment exported

```bash
cd node_autonomos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --autonomos-id 1
```

**Terminal n, peer (either `syndesmos`/`autonomos`)**, with its own storage environment exported
for instance:
```bash
cd node_autonomos
cargo run -- --mesh-a-cluster ../meshes_config/mesh_a_cluster_local.toml --autonomos-id n
```

They expose an interactive prompt:

```
store <file_name> <file_type> <secret> <file_path>
call  <file_name> <file_type> <secret>
get   <file_name> <file_type> <secret> <private_key> <nonce>
```

- **`store`** encrypts the file, computes its MID and CID, and publishes the MID to the
  DHT. It prints the **key and nonce. Record them, the mesh does not keep a copy.**
- **`call`** recomputes the MID from the three metadata values and locates providers and stores
into the caller content and metadata folders and merkle tree.
- **`get`** retrieves and decrypts, given the key and nonce, it will be located at ```./data/decrypt_content```.

```
store briefing .pdf 8f3kQ9zR2vLmXw ./samples/briefing.pdf
call  briefing .pdf 8f3kQ9zR2vLmXw
get   briefing .pdf 8f3kQ9zR2vLmXw <key-hex> <nonce-hex>
```

Use a high-entropy `secret`: anyone who can guess all three metadata values can locate the
content.

## 6. Full stack: confidential compute

Bring the tiers up in order. Each needs its own terminal.

**Mesh B comes first.** Key material has to exist before a party will start, and the
parties have to be running and initialized before the gateway or an executor can do
anything. The order in §6.1 – §6.4 is not a suggestion.

### 6.1 Generate KMS key material (once, before anything else)

Run from `node_kryphos/`, so the key and certificate directories land where the party
configuration expects them:

```bash
cd node_kryphos
```

Generate the threshold key material, meaning signing keys, FHE key shares and the CRS:

```bash
cargo run -p kms --bin kms-gen-keys -F testing -F threshold-fhe/testing -- \
  --private-storage file --private-file-path ./keys \
  --public-storage  file --public-file-path  ./keys \
  threshold
```

`--cmd` narrows what is produced to `all` (the default), `signing-keys`, `fhe-keys` or
`crs`, which is useful when regenerating one piece. `threshold` is the mode; `centralized`
exists for a single-party KMS that ØMYSTIK does not use.

> [!NOTE]
> Threshold mode generates the key shares ***centrally*** and then distributes them. That is
> a testing convenience, not the threshold ceremony: for a deployment where the threshold
> guarantee has to hold, key generation runs as an MPC protocol through Kentr node
> (§6.7), not through this command.

### 6.2 Generate TLS certificates (once)

The MPC cores authenticate each other with TLS. Generate one CA per party. `--ca-count`
must match the number of parties in your `mpc_cluster_*.toml`:

```bash
cargo run -p kms --bin kms-gen-tls-certs -- --ca-prefix p --ca-count 4
```

### 6.3 Mesh B: start the Kryphos parties

One terminal per party. Repeat with `--node-id 2`, `3`, `4`, matching the `[[node]]`
entries in `mpc_cluster_local.toml`:

```bash
cd node_kryphos
RUST_LOG="info,kms_threshold=debug,kms_lib=debug,threshold_fhe=debug" \
  cargo run --bin node_fhe --features "p2p insecure" -- \
  --kryphos-config ../meshes_config/mpc_cluster_local.toml --node-id 1
```

`node_fhe` **is** the KMS server here. It calls the same `run_server` entry point upstream
runs as `kms-server`, wrapped so that the inter-party transport is libp2p.

Each party ends up listening on three things, which is worth knowing when a port looks
wrong:

| Port | From | Carries |
|---|---|---|
| `50100`, `50200`, … (TCP) | `[service] listen_port` in `kms/core/service/config/default_N.toml` | the gRPC service, which is what `kms-init` and the core client talk to |
| `50100`, `50200`, … (UDP/QUIC) | `listen_address` in `mpc_cluster_*.toml` | libp2p MPC rounds between parties |
| `8100`, `8200`, … (UDP/QUIC) | `gateway_listen_addr` in `mpc_cluster_*.toml` | Face B, from the Pylon |

The first two share a number but not a transport, so they do not collide.

Wait until the parties report each other connected before continuing.

### 6.4 Initialise the parties (once per set of nodes)

With all parties up, initialise them. The addresses are the **gRPC service** ports:

```bash
cargo run -p kms --bin kms-init -- \
  -a http://127.0.0.1:50100 \
     http://127.0.0.1:50200 \
     http://127.0.0.1:50300 \
     http://127.0.0.1:50400
```

This is the step most easily got wrong, so four facts about it:

- **Run it exactly once** for a given set of parties. Calling it again returns an error.
- The material it produces is written to each party's private storage under
  `PRIV-p<N>/PrssSetup/`.
- **Restarting a party does not need it again.** A party finds the material on disk and
  reuses it, which is how a failed node rejoins an existing set.
- **Changing the set of parties does.** A different number of parties, or different
  parties, needs fresh init material, and the only way to get it is to delete
  `PRIV-p<N>/PrssSetup/` on every party by hand and run `kms-init` again.

Until this succeeds, public and user decryption will not work, however healthy the parties
look.

### 6.5 Pylon gateway

```bash
cd node_pylon
RUST_LOG=info cargo run --bin node_pylon -- \
  --gateway-cluster ../meshes_config/gateway_cluster_local.toml \
  --mpc-cluster     ../meshes_config/mpc_cluster_local.toml \
  --gateway-id 1
```

Narrower logging for gateway internals: `RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug`

### 6.6 Mesh C: Ypolo executor

```bash
cd node_ypolo
RUST_LOG=libp2p_swarm=debug,libp2p_rendezvous=debug \
  cargo run --bin ypolo -- \
  --ypolo-cluster ../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1
```

### 6.7 Kentr: keys and KMS operations

Fetch and rehydrate an existing keyset:

```bash
cd node_kentr
cargo run --bin kentr -- \
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml \
  --kentr-id 1 \
  --rehydrate-key \
  -- -f config/client_local_threshold.toml
```

Arguments after `--` pass through to Zama's core client. Key generation is two-phase:
preprocessing first, then generation with the resulting id:

Preproc and KeyGen:
```bash
# 1. preprocessing, note the PREPROC_ID it returns
cargo run --bin kentr -- \
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \
  -- -f config/client_local_threshold.toml -a -l preproc-key-gen

# 2. generation
cargo run --bin kentr -- \
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \
  -- -f config/client_local_threshold.toml -a -l key-gen --preproc-id <PREPROC_ID>

# NB
For convenience within 0MYSTIK, these commands have been merged into a single-shot generation: with `-l key-gen-fresh`.
cargo run --bin kentr -- --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 -- -f config/client_local_threshold.toml -a -l key-gen-fresh

#[!Warning]
At client_xxx_threshold.toml, the fhe_params parameters impacts the keys generation time:
- "Test", Small, insecure parameters for testing takes "around 27 to 35 minutes".
- "Default", Large, secure parameters takes at "least 50 hours".
```

CRS generation:

```bash
cargo run --bin kentr -- \
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \
  -- -f config/client_local_threshold.toml -a -l crs-gen --max-num-bits 2048
```

Round-trip check, encrypting a value and decrypting it through the threshold:

```bash
cargo run --bin kentr -- \
  --kentr-cluster ../meshes_config/kentr_cluster_local.toml --kentr-id 1 \
  -- -f config/client_local_threshold.toml -a -l user-decrypt from-args \
     --to-encrypt 0x2342 --data-type euint16 --key-id <KEY_ID>
```

---
<br><br>


# OSATCON

## 1. Running demo

The reference application, on top of a running full stack ([§6](#6)).

**Install the smart program** on the executor. `--runner-command` is the absolute path to
the built `smart-program-alert` binary on the machine hosting Ypolo:

```bash
cargo build -p smart-program-alert --bin smart-program-alert
cargo build -p smart-program-alert --bin deploy_alert

cd osatcon/smart-program-alert
cargo run --bin deploy_alert -- \ 
  --kentr-cluster ../../meshes_config/kentr_cluster.toml \
  --kentr-id 2 \
  --ypolo-cluster ../../meshes_config/mesh_c_cluster.toml \
  --ypolo-id 1 \
  --runner-command /YOUR_PATH/mesh/target/debug/smart-program-alert
```
**NB**: for some reasons, this command may failed if not one line.

**Field terminals.** All participants must share the same `--session-code`:

```bash
# convoy
cd osatcon/edge-convoy-operator
cargo run -p edge-convoy-operator -- \
  --mesh-a-cluster ../../meshes_config/mesh_a_cluster_local.toml --autonomos-id 1 \
  --ypolo-cluster  ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \
  --session-code '<your_choice>'

# satellite
cd osatcon/edge-satellite-operator
cargo run -p edge-satellite-operator -- \
  --mesh-a-cluster ../../meshes_config/mesh_a_cluster_local.toml --autonomos-id 2 \
  --ypolo-cluster  ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \
  --session-code '<your_choice>'
```

**Command view:**

```bash
cd osatcon/headquarter
cargo run -- \
  --kentr-cluster ../../meshes_config/kentr_cluster_local.toml --kentr-id 2 \
  --ypolo-cluster ../../meshes_config/mesh_c_cluster_local.toml --ypolo-id 1 \
  --session-code '<your_choice>'
  -- -f config/client_local_threshold.toml
```

Add `--ui-only` to inspect the interface without a live mesh.

<br>

## 2. Cross-compiling for edge hardware

### [Current cross.toml](../Cross.toml)

The reference deployment deliberately targets old and constrained machines.

**Raspberry Pi Zero 2 W** (armv7) for `syndesmos` and `pylon`:

```bash
CARGO_TARGET_DIR=target-pi cross build --release \
  --target armv7-unknown-linux-gnueabihf \
  -p node_syndesmos --bin node_syndesmos
```

**Raspberry Pi 5** (aarch64) for the convoy operator:

```bash
cross build -p edge-convoy-operator \
  --target aarch64-unknown-linux-gnu \
  --profile release-lto-off --no-default-features -j 1
```

**x86_64 Linux** for the satellite operator:

```bash
cross build -p edge-satellite-operator \
  --target x86_64-unknown-linux-gnu \
  --no-default-features --release -j 1
```

**Older macOS** (10.13) for ypolo and the alert program:

```bash
MACOSX_DEPLOYMENT_TARGET=10.13 cargo build -p node_ypolo \
  --bin ypolo --release --target x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=10.13 cargo build -p smart-program-alert \
  --bin smart-program-alert --release --target x86_64-apple-darwin
```

`release-lto-off` and `-j 1` exist because full LTO exhausts memory on constrained build
hosts. `Cross.toml` holds the per-target settings.

Deployed binaries take the same arguments, with configs alongside:

```bash
./node_syndesmos --mesh-a-cluster mesh_a_cluster.toml --syndesmos-id 1
DISPLAY=:0 ./edge-convoy-operator \
  --mesh-a-cluster mesh_a_cluster.toml --autonomos-id 1 \
  --ypolo-cluster mesh_c_cluster.toml --ypolo-id 1 --session-code '<your_choice>'
```

<br>

# Troubleshooting

**`Failed to dial … quic-v1`**: the target isn't listening, or its address/peer ID in the
cluster file is stale. Rendezvous nodes must be up before anything registers.

**Face B requests fail or hang**: the MPC cluster is incomplete. All parties named in
`mpc_cluster_local.toml` must be running and mutually connected; the gateway's
`num_reconstruct` cannot be satisfied otherwise.

**`get` fails to decrypt**: key and nonce must match the ones `store` printed. They are
not recoverable from the mesh.

**`call` finds no providers**: either the metadata triple doesn't match what was stored
(all three fields, exactly), or the MID has not yet propagated to the DHT.

**Storage paths wrong or shared**: each node needs its own `STORAGE_*` environment. Two
nodes pointed at one tree will corrupt each other's state.

Useful log filters:

```bash
RUST_LOG=libp2p_swarm=debug,libp2p_quic=debug,libp2p_rendezvous=debug   # networking
RUST_LOG="info,kms_lib=debug,threshold_fhe=debug"                        # KMS / MPC
RUST_LOG=faceb=info,node_pylon::runtime=info                             # gateway
```

---

*Architecture: [ARCHITECTURE.md](ARCHITECTURE.md) · Crates: [CRATES.md](CRATES.md) ·
Node data tree: [NODE_LAYOUT.md](NODE_LAYOUT.md)*
