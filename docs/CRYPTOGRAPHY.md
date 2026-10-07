# Cryptographic Inventory

Every cryptographic primitive ØMYSTIK uses, what it protects, and where the keys live.

Written for auditors, integrators, and anyone completing a regulatory declaration about the
software. For how the pieces fit together, read [ARCHITECTURE.md](ARCHITECTURE.md); for the
security posture and its known weaknesses, [SECURITY.md](../SECURITY.md).

> [!NOTE]
> ØMYSTIK implements no cryptographic primitive of its own. Every algorithm below comes
> from an established library: Zama's TFHE-rs and KMS, the RustCrypto suite, BLAKE3,
> rust-libp2p, or rustls with its `aws-lc-rs` provider. What ØMYSTIK contributes is the
> composition: which primitive protects what, and how keys move between nodes.
>
> "Established" carries one qualification. The rustls on the transport path is a third-party
> fork, not the crates.io release. See [§6](#6-provenance).

---

## 1. What the software does, cryptographically

Three separate protections, easily confused because they all involve encryption:

| | Protects | Primitive | Key held by |
|---|---|---|---|
| **Stored content** | Files at rest on a node | XChaCha20-Poly1305 | The user who stored the file, never the mesh |
| **Transport** | Traffic between nodes | TLS 1.3, inside QUIC | Each node, per session |
| **Computation** | Data *while being computed on* | TFHE (fully homomorphic) | Split across MPC parties; never assembled |

The third is the unusual one. Data submitted for computation is encrypted under a threshold
FHE public key, evaluated in ciphertext form by a node that holds only an evaluation key,
and decrypted only when a quorum of independent parties cooperates.

---

## 2. Algorithms

### 2.1 Own crates

| Algorithm | Mode | Key / output size | Function |
|---|---|---|---|
| **XChaCha20-Poly1305** | AEAD, streamed in 2 MiB chunks (`EncryptorBE32` / `DecryptorBE32`) | key 256 bits, nonce 152 bits | Confidentiality + integrity of stored files |
| **BLAKE3** | Hash | 256-bit output | Content identifier (CID) derivation; transfer integrity; sparse Merkle tree hashing |
| **SHA-256** | Hash | 256-bit output | Metadata Merkle tree (`rs_merkle`), producing the MID |
| **Ed25519** | Signature | 256 bits | libp2p peer identity, carried in the QUIC TLS certificate |
| **ECDSA / secp256k1** | Signature | 256 bits | KMS signing keys, artifact signatures |
| **TLS 1.3, inside QUIC** | X25519 key agreement (P-256, P-384 as fallbacks); one of `TLS13_CHACHA20_POLY1305_SHA256`, `TLS13_AES_256_GCM_SHA384`, `TLS13_AES_128_GCM_SHA256` | 256 bits | Transport encryption and peer authentication, on every link including between MPC parties |

> [!NOTE]
> **Noise is not used, despite being a libp2p default that readers expect.** Every swarm in
> this workspace is built `.with_tokio().with_quic()` and nothing else: see
> `libp2p_common/src/komvos.rs`, `ypolo.rs` and `kryphos_dual.rs`. There is no TCP transport
> and therefore no Noise handshake, and every configured multiaddr is `/udp/<port>/quic-v1`.
> The `noise` feature in `Cargo.toml` is enabled but never reaches a code path. Transport
> security is TLS 1.3 as QUIC provides it, with the peer's Ed25519 identity bound in through
> the libp2p certificate extension.

The transport row is exact rather than indicative. `libp2p-tls` 0.5.0 restricts the
connection in three ways: TLS 1.3 only, the three cipher suites named above and no others,
and the **ring** crypto provider. It does not override the key exchange groups, so ring's
defaults apply: X25519 first, then P-256 and P-384. **No hybrid post-quantum group is
offered.** Newer rustls builds on `aws-lc-rs` can negotiate `X25519MLKEM768`, but the ring
provider has no ML-KEM at all, so it cannot arise here. Between two ØMYSTIK nodes the
agreement is X25519.

### 2.2 Vendored Zama KMS

Reachable in the shipped artifact, not merely declared in the manifest. See
[kms/UPSTREAM.md](../kms/UPSTREAM.md) for provenance.

| Algorithm | Mode | Key / output size | Function |
|---|---|---|---|
| **TFHE** | Fully homomorphic encryption | per DKG parameter set | Computation over ciphertext |
| **Threshold secret sharing** | MPC, *n* = 3*t* + 1 | n/a | Distributed key generation and decryption |
| **ML-KEM (FIPS 203)** | Post-quantum KEM | ML-KEM-512 in use; ML-KEM-1024 for legacy key deserialisation only | Hybrid encryption for signcryption and backup |
| **AES-256-GCM** | AEAD | 256 bits | Symmetric encryption within KMS |
| **AES-256-GCM-SIV** | AEAD, nonce-misuse resistant | 256 bits | Keychain / vault protection |
| **AES-based PRNG** | CSPRNG | 256 bits | Deterministic randomness for MPC protocols |
| **ECDSA / secp256k1** | Signature | 256 bits | Core signing key, EIP-712 |
| **ECDSA / P-384** | Signature | 384 bits | Certificate paths |
| **RSA** | Encryption (recipient keypair) | 2048 bits | AWS KMS keychain interoperability |
| **SHA-3** | Hash | n/a | Within KMS protocols |
| **Zero-knowledge proofs** | `tfhe-zk-pok` | n/a | Proofs of correct ciphertext formation |

Presence does not mean every deployment exercises every algorithm: the AWS-facing paths
(RSA, parts of the keychain) are inactive unless AWS KMS or S3 storage is configured.

---

## 3. Key lifecycle

### 3.1 Per-file content keys

Generated by `content_hashing::newkey()` from the operating system CSPRNG (`OsRng`): a
256-bit key and a 152-bit nonce, fresh for every file.

`store_file` returns them to the caller and **the mesh retains no copy.** There is no
recovery path. Losing them makes the content permanently unreadable, and leaking them
exposes that file and no other.

The wider model these keys sit in, MID, CID and the two Merkle layers, is described in
[README_mystik-p2p.md](../README_mystik-p2p.md).

Because the key and nonce are fresh per file, encrypting identical plaintext twice produces
different ciphertext and therefore a different CID. The CID is BLAKE3 over the *ciphertext*,
so it reveals nothing about the plaintext and cannot be used to correlate identical files
held by different nodes.

### 3.2 Threshold FHE keys

Generated by the MPC parties as a distributed protocol. The private key exists only as
shares; reconstruction requires a quorum, and no party ever holds the whole key, not even
the gateway that aggregates their responses.

Three artifacts are distributed outward from that process:

| Artifact | Distributed to | Enables |
|---|---|---|
| `PublicKey` | any node that encrypts | Encrypting inputs |
| `ServerKey` | compute executors (Ypolo) | Evaluating over ciphertext, **not** decryption |
| `PublicKeyMetadata` | control nodes | Parameter agreement |

Acquisition is described in [ARCHITECTURE.md §2](ARCHITECTURE.md#how-nodes-find-each-other):
originals come from the Pylon gateway over Face A `GetArtifact`; replicas propagate between
Mesh A peers over `/key/1`.

### 3.3 Node identities

Each node has a libp2p Ed25519 keypair. In the committed example configurations these are
derived from a one-byte `secret_seed`, which is **reproducible by anyone** and suitable only
for local testing. See
[SECURITY.md](../SECURITY.md#deterministic-node-identities-from-short-seeds).

### 3.4 Transport and MPC certificates

**MPC parties authenticate each other the same way every other pair of nodes does**, through
QUIC's TLS 1.3 and the Ed25519 peer identity carried in the libp2p certificate. There is no
separate authentication mechanism for Mesh B.

The upstream KMS protects the party-to-party link with mTLS over gRPC instead, and that
machinery is still present: `kms-gen-tls-certs` generates one CA per party, the shipped
`kms/core/service/config/default_*.toml` carry a `[threshold.tls.manual]` block with
`cert_p1.pem` … `cert_p4.pem`, and `build_tls_config` in `node_kryphos/src/lib.rs` reads them
at startup. **None of it is in effect.** `kms_impl.rs` selects the transport at compile time,
and the TLS configuration is consumed only under `#[cfg(not(feature = "p2p"))]`, while the
default build is `default = ["p2p", "insecure"]`. On the p2p branch the certificates are
built, passed to `new_real_threshold_kms`, and never used.

Two consequences worth knowing if you operate this:

- The TCP socket bound by `make_mpc_listener` is never served under p2p. It must still bind,
  or the process panics, for a listener nothing reads.
- The log line *"No TLS identity, using plaintext communication between MPC nodes"* is
  misleading here. Omitting TLS configuration does not produce plaintext, it produces
  QUIC-encrypted traffic.

---

## 4. Data handling around encryption

### 4.1 Before encryption

No compression, no format conversion, no header insertion. Plaintext is read from disk and
streamed into the AEAD in 2 MiB chunks; it is never held in memory in full.

### 4.2 After encryption

The ciphertext is written as `<CID>.bin`, where the CID is the BLAKE3 hash of the ciphertext
stream. No header, no envelope, no metadata is attached to the encrypted file; the
metadata lives separately, in the Merkle structures described in
[ARCHITECTURE.md §4](ARCHITECTURE.md#4-how-file-storage-and-exchange-works).

For transport, ciphertext is framed by the libp2p protocol in use (`/file-exchange/1` for
files, `/mesh/gateway/blob/1` for compute blobs) and hashed with BLAKE3 at both ends.

FHE ciphertexts use TFHE safe serialization
(`tfhe.safe_serialize.CompactCiphertextList.v1`), which carries its own versioned header.

---

## 5. Protecting the encryption process

What exists today, stated plainly:

- **Streaming AEAD.** Each chunk is independently authenticated, so truncation or
  reordering is detected rather than silently decrypted.
- **Transfer verification.** BLAKE3 on both sides of every file transfer.
- **Signed artifacts.** Key artifacts and program manifests carry signatures, verified
  through `verify_signed_artifact`.
- **Threshold custody.** No single party can decrypt, by construction rather than by
  policy.

What does not exist: no hardware key storage (the vendored KMS carries an AWS Nitro enclave
path, which no ØMYSTIK configuration enables), no tamper detection on the software itself,
and no runtime integrity checking of the binary.

Six TLS-related dependencies come from third-party git branches rather than crates.io. To be
precise about what that costs, because it is easy to overstate: `Cargo.lock` does record an
exact commit for each, so a build from the committed lockfile is deterministic. The exposure
is that the *manifest* names a mutable branch, so `cargo update` silently follows wherever
that branch has moved; that reproducing any build depends on a third party keeping those
commits reachable; and that the code itself is unaudited and outside the crates.io supply
chain. The root and `kms/` lockfiles already disagree on three of them. See
[§6](#6-provenance) and
[SECURITY.md](../SECURITY.md#cryptographic-dependencies-use-forks).

---

## 6. Provenance

| Source | Supplies |
|---|---|
| [Zama TFHE-rs](https://github.com/zama-ai/tfhe-rs) 1.4.0-alpha.3 | FHE |
| [Zama KMS](https://github.com/zama-ai/kms) v0.12.3, vendored | Threshold key management, MPC |
| [rust-libp2p](https://github.com/libp2p/rust-libp2p) 0.54.1 | QUIC transport, peer identity, `libp2p-tls` 0.5.0 and `libp2p-quic` 0.11.1 |
| `rustls` 0.23.31, **from a git branch**, with `quinn` 0.11.9 | The TLS 1.3 protocol behind QUIC; primitives come from its provider, `ring` |
| `ring` 0.17.14 | rustls' crypto provider **on the transport path**: X25519, the TLS 1.3 AEADs, certificate signature verification |
| `aws-lc-rs` 1.15.1 | rustls' crypto provider on the KMS mTLS path, installed as the process default in `node_kryphos` |
| [RustCrypto](https://github.com/RustCrypto) | ChaCha20-Poly1305, AES-GCM, AES-GCM-SIV, SHA-2, SHA-3, ML-KEM, k256, p384, RSA |
| [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) | Hashing |

> [!WARNING]
> **The `rustls` on the transport path is not the crates.io one.** `[patch.crates-io]`
> redirects it to `mkmks/rustls` branch `k256`, revision `2e446f8`, and `Cargo.lock` shows
> `quinn`, `quinn-proto` and `libp2p-tls` all resolving to that patched `0.23.31`. QUIC being
> the only transport, this means every handshake and every encrypted exchange between every
> pair of nodes, including between MPC parties, is driven by a TLS implementation on a
> mutable third-party branch that is outside crates.io and unaudited. The primitives
> themselves come from `ring`, unpatched; what the fork controls is the protocol around
> them, which is enough to matter.
>
> The same applies to `rustls-pki-types`, `rustls-webpki`, `tokio-rustls`, `rcgen` and
> `attestation-doc-validation`. See
> [SECURITY.md](../SECURITY.md#cryptographic-dependencies-use-forks).

**What the fork actually changes.** The branch name is `k256`, and it does what the name
says: it teaches the TLS certificate stack ECDSA over **secp256k1**, exposed as
`webpki::aws_lc_rs::ECDSA_P256K1_SHA256` and rcgen's `PKCS_ECDSA_P256K1_SHA256`. Neither
exists upstream, because secp256k1 is not an IANA-registered TLS signature scheme. Zama needs
it so a party's TLS certificate can be signed with the same secp256k1 key the KMS already
uses for EIP-712. The underlying primitives remain a provider's; the fork adds which
signature algorithm the certificate path will accept, not new arithmetic.

**Two providers are in the build, and they serve different paths.** `node_kryphos` installs
`aws-lc-rs` as the process-wide default, which is what the mTLS path picks up, and the
secp256k1 additions are exposed as `webpki::aws_lc_rs::ECDSA_P256K1_SHA256`. But
`libp2p-tls` does not use the process default: it constructs its own provider from **ring**
explicitly. So the transport, which is the only path that runs, is served by ring, and
`aws-lc-rs` serves the path that does not.

There is an irony worth recording. Every call site for the secp256k1 additions sits on the
mTLS path, in `kms/core/threshold/src/tls_certs.rs`, `kms-server.rs` and `build_tls_config`,
and §3.4 explains that path is not the one ØMYSTIK runs. So the forked TLS stack is carried
across the entire transport to support a feature only the unused branch needs. Reverting to
crates.io rustls is therefore worth *investigating* rather than assuming impossible, though
the patch is inherited from the vendored KMS and both workspaces would have to agree.

Full licence terms and the Zama commercial-use obligation: [LICENSING.md](../LICENSING.md).

---

## 7. Status

ØMYSTIK has **not** undergone independent cryptographic review, and the default build
enables an `insecure` feature that relaxes protections in the KMS. Nothing in this document
should be read as an assurance that the composition is correct, only as an accurate
description of what it is. See [SECURITY.md](../SECURITY.md).
