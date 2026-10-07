# Licensing

ØMYSTIK is licensed **BSD-3-Clause-Clear**, matching the license of the ZAMA components it
builds on.

---

## 1. ØMYSTIK's own license

BSD-3-Clause-Clear (the "Clear BSD License") is a permissive license with one notable
addition over plain BSD-3-Clause: it states explicitly that **no patent rights are
granted**. You get broad freedom to use, modify, and redistribute the code, including
commercially, provided you retain the copyright notice, the license text, and the
disclaimer, and do not use the authors' names to endorse derived products.

Copyright holder: Michael John Hatchi.

Full text: <https://spdx.org/licenses/BSD-3-Clause-Clear.html>

## 2. ZAMA components: commercial use requires an agreement

ØMYSTIK vendors and links ZAMA's key-management and FHE stack. This is the single most
important licensing obligation in the project, and it is easy to overlook because the
license itself is permissive.

**ZAMA's BSD-3-Clause-Clear licensing of TFHE-rs, Concrete, Concrete-ML, threshold-FHE and
the KMS applies to non-commercial and evaluation use. ZAMA requires a separate commercial patent
license agreement for commercial use. Commercial terms, including any applicable usage measurement and remuneration mechanism, are agreed directly with ZAMA.**

If you intend to build a product or service on ØMYSTIK, contact ZAMA and conclude that
agreement. It is your obligation, not the ØMYSTIK maintainers'. A permissive license on
this repository does not and cannot waive ZAMA's terms. See <https://www.zama.ai/> and
the license text shipped with each ZAMA component.

### Vendored KMS

The `kms/` directory contains a vendored copy of ZAMA's KMS, taken from:

> <https://github.com/zama-ai/kms/tree/v0.12.3>

It is not a submodule; it is a copy carrying local modifications for libp2p transport
(see [ARCHITECTURE.md](docs/ARCHITECTURE.md)).

Consequences:

- ZAMA's copyright notice and license text **must** ship with it.
- Upstream security fixes do not arrive automatically. Track
  <https://github.com/zama-ai/kms> releases and port fixes deliberately.

ZAMA components in use, via the workspace manifest:

| Component | Version | Source |
|---|---|---|
| `kms` (core service) | vendored | `kms/core/service`, upstream v0.12.3 |
| `kms-grpc` | vendored | `kms/core/grpc` |
| `threshold-fhe` | vendored | `kms/core/threshold` |
| `observability` | vendored | `kms/observability` |
| `bc2wrap` | vendored | `kms/bc2wrap` |
| `tfhe` | 1.4.0-alpha.3 | crates.io |
| `tfhe-csprng` | 0.7.0 | crates.io |
| `tfhe-versionable` | 0.6.2 | crates.io |
| `tfhe-zk-pok` | 0.7.3 | crates.io |

## 3. Other third-party components

| Component | License | Role |
|---|---|---|
| [rust-libp2p](https://github.com/libp2p/rust-libp2p) 0.54.1 | MIT | P2P transport, DHT, rendezvous, streams, request-response |
| `chacha20poly1305` (RustCrypto) | Apache-2.0 OR MIT | XChaCha20Poly1305 content encryption |
| `blake3` | Apache-2.0 OR CC0-1.0 | Content and transfer hashing |
| `rs_merkle` | MIT | Classic Merkle tree for metadata (MID) |
| `monotree` | MIT | Sparse Merkle tree for MID→CID mapping |
| `sled` (via monotree, `db_sled`) | Apache-2.0 / MIT | Embedded persistence backend for the sparse Merkle tree |
| `tokio`, `tower`, `tonic`, `tracing` | MIT | Async runtime, gRPC, instrumentation |
| `ndarray` | Apache-2.0 OR MIT | Numeric arrays |
| W3C DID specifications | W3C Document License | Identity data model (planned; not implemented) |

### Supply-chain annotations in the manifest

`Cargo.toml` annotates most dependencies with a maintainer-risk assessment
(`LOW RISK: RustCrypto org`, `HIGH RISK: Individual maintainer`, and so on). These are the
maintainer's own judgements, inherited in part from ZAMA's manifest.

## 4. Contributor licensing

By submitting a contribution you agree it is licensed under BSD-3-Clause-Clear and that
you have the right to submit it. There is no separate CLA. See
[CONTRIBUTING.md](CONTRIBUTING.md).

Do not contribute code you are not permitted to publish, export-controlled material in
particular. See [DUAL_USE.md §5](DUAL_USE.md#5-export-control-and-sanctions).

## 5. Where the license texts live

| File | Covers |
|---|---|
| [LICENSE](LICENSE) | ØMYSTIK's own code, BSD-3-Clause-Clear |
| [kms/LICENSE](kms/LICENSE) | ZAMA's KMS, vendored under `kms/`. BSD 3-Clause Clear, © 2024 ZAMA |
| [kms/UPSTREAM.md](kms/UPSTREAM.md) | What in `kms/` diverges from upstream v0.12.3, and who to report defects to |

If you redistribute ØMYSTIK, both license files travel with it. Every other dependency is
permissively licensed, see [§3](#3-other-third-party-components).<br>
So the only obligation that needs a decision rather than a notice is ZAMA's commercial-use agreement in
[§2](#2-zama-components-commercial-use-requires-an-agreement).

---

*Responsible-use terms are in [DUAL_USE.md](DUAL_USE.md); security posture in
[SECURITY.md](SECURITY.md).*
