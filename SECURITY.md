# Security Policy

## Status: pre-production, unaudited

ØMYSTIK is under active development and has **not** undergone independent security audit
or cryptographic review.<br>
The maintainer does not currently make a security guarantee for any release.
Treat every version as experimental.<br>

Do not use ØMYSTIK to protect data whose disclosure would harm someone.

---

## Reporting a vulnerability

Report privately. **Do not open a public issue for a security defect.**

- Email: **support@omystik.org** with `[ØMYSTIK SECURITY]` in the subject line.
- If you use GitHub Security Advisories, open a private advisory on the repository
  instead.

Please include:

- affected component (crate, node type, protocol) and version or commit;
- what an attacker gains, and what access they need to start;
- reproduction steps or a proof of concept;
- your assessment of severity.

**What to expect.** ØMYSTIK is currently maintained by one person, so response is
best-effort rather than SLA-backed. Expect acknowledgement within about a week. If you
have had no reply in two weeks, send a follow-up before considering any disclosure.

**Disclosure.** Please allow 90 days from acknowledgement before public disclosure, or
until a fix ships, whichever is sooner. If a defect is being actively exploited, say so
prominently, because that changes the handling. Reporters are credited unless they ask not to be.

**Scope.** Defects in ØMYSTIK's own crates are in scope. Defects in upstream dependencies,
notably Zama's KMS and TFHE-rs and rust-libp2p, should be reported to those projects;
tell me as well if ØMYSTIK's usage makes the impact worse than upstream's own assessment.

---

## Known limitations

These are known and unresolved. They are listed so that no one deploys under a
misapprehension. Several are deliberate development conveniences that **must** be removed
before any production use.

### `insecure` is a default feature

The workspace manifest enables it by default:

```toml
[features]
default = ["p2p", "insecure"]
insecure = ["kms/insecure"]
```

This propagates Zama KMS's `insecure` feature, which enables key-generation and
decryption paths that skip protections intended for production. Documented run commands
pass `--features "p2p insecure"` routinely.

**Consequence:** a default `cargo build` produces binaries that are not safe for
production. Any hardened build must disable it explicitly and be verified to still work.

### Deterministic node identities from short seeds

Cluster configuration files assign each node a `secret_seed`, a single byte in the
committed examples:

```toml
[[syndesmos]]
syndesmos_id = 1
secret_seed = 218
```

Node keypairs derived from a one-byte seed are trivially enumerable: an attacker who
knows the derivation can reproduce any node's private key by trying 256 values. This is
acceptable for reproducible local testing and unacceptable anywhere else.

**Consequence:** every peer identity in every committed config must be treated as public.
Production deployments need real generated keys held outside the config files.

### Threshold guarantees depend on party independence

Threshold FHE splits trust across MPC parties (`num_majority` / `num_reconstruct` in the
gateway configuration). A cluster is sized by **$n = 3t + 1$**, where $t$ is the maximum
number of parties that may be compromised, so the standard 4-party cluster tolerates one.

Two things follow, and only the first is enforced by configuration:

- Exceeding $t$ compromised parties breaks the guarantee outright.
- The guarantee holds only if no single actor controls a reconstructing subset. Party
  *count* is configured; party *independence* is an operational property that nothing in
  the code can check for you.

Local and test clusters run all parties on one host, which provides **no** threshold
guarantee. It is a functional harness, not a security configuration.

### Transport and identity

- The mesh runs over QUIC and nothing else, and authenticates transport with QUIC's TLS 1.3
  and the Ed25519 peer identity carried in the libp2p certificate. No TCP transport is
  configured, so libp2p Noise never runs. This authenticates *peers*, not the *humans or
  agents* behind them.
- **The MPC parties have no additional protection.** The upstream KMS protects the
  party-to-party link with mTLS over gRPC, and that configuration still ships in
  `kms/core/service/config/default_*.toml`, but it is consumed only under
  `#[cfg(not(feature = "p2p"))]` and the default build enables `p2p`. The certificates are
  generated, loaded and ignored. Mesh B is protected exactly as Mesh A is, no more.
- **DID-based access control is not implemented yet.** Zero-Trust identity via W3C DIDs is
  design intent recorded in the project roadmap, not shipped code. There is presently no
  node-level authorisation layer beyond peer allowlists such as `allowed_gateway_peers`.
- Rendezvous (`syndesmos`) nodes are discovery choke points. A hostile or spoofed
  rendezvous node can partition or misdirect peers. Bootstrap peer IDs must be
  distributed out of band and pinned.
- No denial-of-service mitigation is implemented. This is a known open item.

### Content and metadata

- Content encryption is XChaCha20Poly1305 with a per-file key and nonce.<br>
**Key custody is the caller's problem.** `store_file` returns the key and nonce and the mesh does not
  retain them.<br>
  Lose them and the content is unrecoverable; leak them and the content is
  exposed.
- Encrypted content is hashed and provides CID.
- Metadata privacy rests on preimage resistance.<br>
A file is addressed by a Merkle root (MID) computed from `file_name`, `file_type`, and a `secret`.
**Low-entropy secrets are guessable**: guess all three and you can compute the MID and find
who holds the file.
- Local node stores the MID as key and the corresponding CID as leaf in a sparse Merkle tree.
- MIDs are published to the DHT with the corresponding peer, so mesh membership reveals
  *that* a node holds content matching a given MID. Content and metadata stay encrypted;
  the fact of holding it does not.


### Cryptographic dependencies use forks

```toml
[patch.crates-io]
attestation-doc-validation = { git = 'https://github.com/mkmks/attestation-doc-validation.git', ... }
rcgen        = { git = 'https://github.com/mkmks/rcgen.git',        branch = 'k256' }
rustls       = { git = 'https://github.com/mkmks/rustls.git',       branch = 'k256' }
rustls-pki-types = { git = 'https://github.com/mkmks/pki-types.git', branch = 'k256' }
rustls-webpki    = { git = 'https://github.com/mkmks/webpki.git',    branch = 'k256' }
tokio-rustls     = { git = 'https://github.com/mkmks/tokio-rustls.git', branch = 'k256' }
```

These are inherited from Zama's KMS, which needs secp256k1 support in the TLS certificate
stack so a party's certificate can be signed with the same key the KMS uses for EIP-712.
They place TLS and attestation validation, which is security-critical code, on unaudited
third-party branches outside the crates.io supply chain, specified by **moving branch** in
the manifest rather than by immutable revision.

**Consequences.** Stated precisely, because the reproducibility part is easy to overstate:

- `Cargo.lock` does record an exact commit for each, so a build from the committed lockfile
  is deterministic. What is not pinned is the *manifest*, so any `cargo update` silently
  follows wherever those branches have since moved.
- Reproducing an old build depends on a third party keeping those commits reachable. If a
  repository is deleted, renamed or history-rewritten, the build stops being buildable.
- A compromise of those repositories compromises TLS in this project.

The last consequence is broader than it first appears. The patch is global, so it is not
only the KMS that gets the forked stack: `Cargo.lock` shows `quinn`, `quinn-proto` and
`libp2p-tls` all resolving to the patched `rustls 0.23.31`. Since QUIC is the only transport
the mesh uses, **every handshake and every encrypted exchange between every pair of nodes is
driven by a TLS implementation living on a mutable third-party git branch.** The primitives
come from `ring` and are unpatched; what the fork controls is the protocol that uses them,
including key agreement and certificate verification. Whoever controls that branch
controls the transport security of the whole mesh, and a force-push changes what your next
`cargo update` links against without changing anything you wrote.

Worth knowing before anyone tries to remove the patch: the secp256k1 additions are used only
on the mTLS path, which the default `p2p` build does not take. See
[CRYPTOGRAPHY.md §3.4](docs/CRYPTOGRAPHY.md#34-transport-and-mpc-certificates). The forked
stack is therefore carried across the whole transport for a feature the running
configuration never reaches.

This is not hypothetical. ØMYSTIK and the vendored KMS are separate Cargo workspaces with
separate lockfiles, and the branches moved between the two resolutions, so the lockfiles
now disagree about half of these crates:

| Crate | root `Cargo.lock` | `kms/Cargo.lock` |
|---|---|---|
| `rustls` | `2e446f8` | `71a29c7` |
| `rustls-pki-types` | `8dd57cb` | `6fe2ed5` |
| `rustls-webpki` | `9ae0560` | `5cfa16a` |

The TLS stack therefore differs depending on which workspace root a build starts from, and
nothing records either resolution as deliberate. Pin exact revisions in both manifests, and
track whether the changes have been upstreamed.

### Panics on untrusted input

Several code paths use `expect`/`assert!` on values derived from network input or
filesystem state. In a node process a panic is a denial of service. This has not been
systematically reviewed.

---

## Hardening checklist

**For anyone running ØMYSTIK in production**, meaning a system carrying real data rather than a
local cluster, a test bench, or an evaluation deployment. Every item is controlled by
whoever operates that system, and none of it happens by default: the defaults are the
unsafe ones.

Before going live:

- [ ] Build with `--no-default-features` plus only the features you need; confirm
      `insecure` is absent from the dependency graph.
- [ ] Replace every `secret_seed` with a real generated keypair stored outside version
      control.
- [ ] Distribute MPC parties across independent operators and hosts; document who runs
      which.
- [ ] Distribute and pin rendezvous peer IDs out of band.
- [ ] Use high-entropy values for the metadata `secret`.
- [ ] Establish key custody for per-file encryption keys and nonces.
- [ ] Confirm the `[patch.crates-io]` entries resolve to pinned revisions rather than
      moving branches, and that you trust the repositories they point at.
- [ ] Obtain independent security review.

---

*Responsible-use scope and export-control considerations are in
[DUAL_USE.md](DUAL_USE.md). Licence terms are in [LICENSING.md](LICENSING.md).*
