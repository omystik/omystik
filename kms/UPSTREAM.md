# Upstream provenance of `kms/`

> **This file is written and maintained by ØMYSTIK. It is not a Zama document.**
> Every other Markdown file in this directory, including `README.md`, `SECURITY.md` and
> everything under `docs/`, belongs to Zama and is carried verbatim. Where they and this
> file disagree, this file describes what ØMYSTIK actually ships.

This directory is a **vendored and modified copy of Zama's KMS**, not a dependency fetched
from crates.io and not a git submodule. Read this before relying on anything in it.

| | |
|---|---|
| Upstream project | [zama-ai/kms](https://github.com/zama-ai/kms) |
| Version vendored | **v0.12.3**, <https://github.com/zama-ai/kms/tree/v0.12.3> |
| Upstream licence | BSD 3-Clause Clear. See [LICENSE](LICENSE), © 2024 ZAMA |
| Modified by ØMYSTIK | yes. See [What diverges](#what-diverges) |

`LICENSE`, `README.md`, `SECURITY.md` and `docs/` are upstream files, carried **verbatim**
from the v0.12.3 tag. They are unedited, so they can still be diffed against upstream.
Where they describe behaviour ØMYSTIK has changed, this file is the correction, and the
upstream text is left alone deliberately.

---

## What diverges

**The transport between MPC parties.** Upstream, KMS parties communicate over gRPC.
ØMYSTIK replaces that with a libp2p transport so parties can take part in the mesh on the
same fabric as every other node, over links that come and go. The substitution lives partly
here and partly in `node_kryphos` and `libp2p_common::kryphos_dual`.

That single change has wide reach, because the gRPC assumption is not confined to a
transport module. It shapes naming and structure throughout the upstream codebase, so many
files were modified and others added.

**The mTLS described in the upstream documentation is not what ØMYSTIK runs.**
[`docs/explanations/network_doc.md`](docs/explanations/network_doc.md) describes parties
authenticating each other with mutual TLS over gRPC, which is accurate for upstream. Here,
`kms_impl.rs` picks the networking manager at compile time, and the TLS configuration is
consumed only under `#[cfg(not(feature = "p2p"))]`. Since the workspace default is
`default = ["p2p", "insecure"]`, the shipped build takes the libp2p branch, where no gRPC
server starts at all.

The certificate machinery still runs: `kms-gen-tls-certs` generates a CA per party,
`core/service/config/default_*.toml` carries a `[threshold.tls.manual]` block, and
`build_tls_config` loads the certificates at startup. They are then passed to
`new_real_threshold_kms` and never used. A reader following the upstream document would
expect those certificates to be doing work they are not. What protects the party-to-party
link is QUIC's TLS 1.3 and the libp2p Ed25519 peer identity, the same as every other link
in the mesh. See [CRYPTOGRAPHY.md §3.4](../docs/CRYPTOGRAPHY.md#34-transport-and-mpc-certificates).

**Nothing upstream was deleted.** Original code sits alongside the ØMYSTIK modifications.
A file existing here therefore does not mean it is on a live path, and a component whose
name says "gRPC" may be any of four things:

- a genuine gRPC component, still in use;
- a larger tool that merely carries gRPC in its name;
- code superseded but left in place;
- a component whose behaviour was replaced underneath the original name.

Check which before relying on it, and before removing it.

### What was added

Upstream v0.12.3 has `core/grpc`, `core/service` and `core/threshold`. This tree adds:

| Path | Size | Role |
|---|---|---|
| `core/threshold/src/networking/p2p/` | 6 files, ~1250 lines | The libp2p networking layer itself: node, router, protocol, sending service, and the crypto gateway service |
| `core/api` (`kms-api`) | 8 files, ~2300 lines | Shared identifiers, RPC types and Solidity types. Depended on by upstream's own `core/service` and `core-client`, and by six ØMYSTIK crates: `node_kryphos`, `node_kentr`, `kryptografia`, `plegma-fhe-pelatis`, `headquarter` and `mesh_gateway_wire` |
| `core/libp2p` (`kms-libp2p`) | 2 lines | A re-export shim over `kms-api`. **Nothing currently depends on it** |

`core/api` and `core/libp2p` are listed in this directory's own `[workspace] members`, so
they build as part of the KMS workspace rather than ØMYSTIK's.

### What was modified

Thirteen upstream files carry ØMYSTIK changes, concentrated where the transport is chosen
and driven:

**`core/threshold/src/networking/`**, the transport layer, opened up so a second
implementation could sit beside the gRPC one:

- `grpc.rs`
- `local.rs`
- `manager.rs`
- `mod.rs`
- `sending_service.rs`
- `session_runtime.rs`
- `value.rs`

**`core/threshold/src/execution/runtime/`**

- `party.rs`

**`core/service/`**, where a party is constructed and served:

- `src/lib.rs`
- `src/bin/kms-server.rs`
- `src/engine/threshold/service/initiator.rs`
- `src/engine/threshold/service/kms_impl.rs`

**`core-client/`**

- `src/lib.rs`

`core/grpc` was also modified, in support of the `core/api` split.

> [!WARNING]
> **This list is not exhaustive.** Treat it as a starting point for an audit rather than a
> complete record of the divergence.

`core/service/src/bin/kms-server.rs` is on the list for a mechanical reason worth knowing:
ØMYSTIK added a `p2p_inputs` parameter to `new_real_threshold_kms()`, so every caller had to
be updated. `kms-server` passes `None` and runs the KMS without libp2p; `node_fhe` passes the
inputs and runs it on the mesh. Both remain valid entry points, for different transports.

## Known gaps

Only part of the upstream repository is vendored here, so a few links in the upstream
documentation point at files this project does not carry:

| In | Reference |
|---|---|
| `docs/guides/core_client.md` | `../../docker-compose-core-threshold.yml`, `../../docker-compose-core-centralized.yml` |
| `docs/tutorials/showcase.md` | `../../docker-compose-core-threshold.yml` |

Those compose files drive Zama's own deployment of the KMS. To run ØMYSTIK, follow
[GETTING_STARTED.md](../docs/GETTING_STARTED.md) instead.

## Where to report problems

The upstream [SECURITY.md](SECURITY.md) in this directory routes vulnerability reports to
Zama's advisory page. That is correct **only for defects that exist upstream too**.

- A defect in ØMYSTIK's modifications → report through
  [the repository's own security policy](../SECURITY.md). Zama can neither reproduce nor fix
  these, and did not introduce them.
- A defect present in unmodified upstream v0.12.3 → report to Zama, and please tell ØMYSTIK
  as well if the way this project uses the code makes the impact worse.

The same applies to the support statement in that file: "only v0.12.0 or newer is supported"
is Zama's policy about Zama's releases. Newer KMS versions exist upstream. **ØMYSTIK pins
v0.12.3 and upstream fixes do not arrive automatically**, so they have to be ported here by
hand.

## Licence obligations

Retaining `LICENSE` is a condition of redistributing this code, not a courtesy. Beyond that,
Zama's licensing distinguishes research and development use from commercial use: commercial
use requires an agreement with Zama covering usage measurement. See
[LICENSING.md](../LICENSING.md) at the repository root. That obligation is yours as a user
of this software; ØMYSTIK's own licence does not and cannot waive it.
