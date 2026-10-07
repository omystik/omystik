# Node Runtime Layout

A ØMYSTIK node keeps its state on disk in a specific directory tree. This document
describes that tree, what creates each part, and how nodes are kept from colliding.

---

## 1. Storage node tree

> `nodeA/` and `nodeN/` at the repository root **are not crates.** They are worked examples
> of this tree, one per node. When running several nodes on one machine, use them and
> create more as needed.

A node that stores encrypted content needs this structure to exist:

```
nodeA/
├── content/                 encrypted files, named by CID  (STORAGE_DIR)
├── metadata/
│   ├── metamerkle/          classic Merkle trees per MID   (STORAGE_MERKLE, STORAGE_METADATA)
│   └── midcid/              sparse Merkle tree: MID → CID  (STORAGE_MIDCID)
└── blob/                    Face A blob staging            (BLOB_STORAGE_DIR)
    ├── acks/                pinned-blob acknowledgements
    └── pubkeys/             fetched FHE key artifacts
```

Nothing creates this tree for you at first run. **Create it before starting a node**, or
the node will fail on paths that don't exist.

```bash
mkdir -p nodeA/{content,metadata/{metamerkle,midcid},blob/{acks,pubkeys}}
```

## 2. Environment variables

The tree is located entirely through the environment. There is no config-file equivalent:
if a variable is unset, the code falls back to a default under `./data/`.

| Variable | Points at | Default | Read by |
|---|---|---|---|
| `STORAGE_DIR` | `content/` | `./data/content/` | `content_hashing::get_storage_dir` |
| `STORAGE_MERKLE` | `metadata/metamerkle/` | `./data/metadata/metamerkle/` | `metadata_mrkl::get_storage_merkle` |
| `STORAGE_MIDCID` | `metadata/midcid/` | `./data/metadata/midcid/` | `metadata_mrkl::get_storage_midcid` |
| `STORAGE_METADATA` | `metadata/metamerkle/` | none | node binaries |
| `BLOB_STORAGE_DIR` | `blob/` | `./data/blob/` | `libp2p_common::komvos::get_storage_blob`, `ypolo` |
| `KEYS_DIR` | key material | `./data/core/keys/` | `content_hashing::get_keys_dir` |

Set them per shell, before launching each node:

```bash
export STORAGE_DIR="./nodeA/content/"
export STORAGE_MIDCID="./nodeA/metadata/midcid/"
export STORAGE_MERKLE="./nodeA/metadata/metamerkle/"
export STORAGE_METADATA="./nodeA/metadata/metamerkle/"
export BLOB_STORAGE_DIR="./nodeA/blob/"
```

> [!CAUTION]
> **Two nodes must never share one tree.** The sparse Merkle tree uses an embedded
> key-value store (`sled`) that assumes a single writer; two processes pointed at the same
> `midcid/` will corrupt each other's state. One tree per node, always. This is the whole
> reason `nodeA` … `nodeN` exist as separate directories.

Because the paths are relative by default, they resolve against each node's **working
directory**. Running `cargo run` from inside `node_autonomos/` while exporting
`./nodeA/content/` resolves to `node_autonomos/nodeA/content/`, not the repository root.
Use absolute paths if that ambiguity bites.

## 3. What lives where

### `content/`: encrypted content
Ciphertexts produced by `content-hashing`, each named by its **CID** with a `.bin`
extension. Written by `encrypt_file`, read by `decrypt_file`. Plaintext never appears here;
the encryption is streaming, so the plaintext is never fully in memory either.

### `metadata/metamerkle/`: metadata Merkle trees
One subdirectory per **MID**, holding the persisted classic Merkle tree (`rs_merkle`,
SHA-256) whose leaves are the file's `file_name`, `file_type`, and `secret`. Presence of a
MID directory is what lets a node prove it holds metadata matching a query without
revealing the metadata.

`db-access` reconstructs MID/CID pairs by reading directory names here against `.bin` names
in `content/`, a recovery path when the sparse tree is lost (when the node is switched off,
for instance).

### `metadata/midcid/`: the MID → CID map
The sparse Merkle tree (`monotree`, Blake3 hashing, `sled` backend) mapping each MID to its
CID. This is the authoritative index: given a MID, it yields the ciphertext to serve. The
pairing is described in [README_mystik-p2p.md](../README_mystik-p2p.md), which documents the
original MYSTIK>p2p storage model.

### `blob/`: Face A staging
Blobs transferred over `/mesh/gateway/blob/1`, addressed by a Blake3-derived `BlobId`.
Compute inputs too large to inline in a `ComputeJobSpec` land here on their way to an
executor.

- **`acks/`** holds acknowledgements that a gateway has pinned a blob (`Client::ack_pinned`),
  so a client knows its upload is durable before submitting the job that depends on it.
- **`pubkeys/`** holds FHE key artifacts, in the layout `key_ops` expects:
  `PUB-p<N>/<KeyType>/<key_id>`, where `KeyType` is `PublicKey`, `PublicKeyMetadata`, or
  `ServerKey`. They arrive either from the Pylon via Face A `GetArtifact` (a Kentr running
  `--rehydrate-key`) or as a replica pulled from a peer over `/key/1`; never from the DHT.
  `unique_public_key_id_from_pub_folders` walks every `PUB-p*` directory and confirms all
  parties report the same key id. A mismatch means the node holds an inconsistent keyset
  and must re-fetch.

## 4. Gateway store: a different tree

`node_pylon` does not use the node tree above. It keeps a `JobStore` under the `data_dir`
from its cluster config (`gateway.face_a.data_dir`, e.g. `./data/gateway`), created on
first open:

```
data/gateway/
├── jobs/                    job records and state
├── blobs/                   blob content
├── acks/                    pin acknowledgements
├── programs/                installed program packages
├── program_refs/            program id/version references
├── artifact_aliases/        named aliases → artifact ids
└── artifact_manifests/      signed artifact manifests
```

Unlike the node tree, `JobStore::open` creates all of these itself.

`node_ypolo` similarly keeps program and artifact state under its own data directory, plus
`BLOB_STORAGE_DIR` for staged inputs.

## 5. Which directories are disposable

Useful when copying, backing up, or packaging a repository.

| Directory | Disposable? | |
|---|---|---|
| `target/`, `target-pi/` | Yes | Build output |
| `jobs/`, `blobs/`, `artifacts/` | Yes | Gateway/executor runtime state; rebuilt on demand |
| `content/` | **No** | Encrypted content; losing it loses the data |
| `metadata/` | **No** | Losing `midcid/` orphans content; `metamerkle/` can partly rebuild it via `db-access` |
| `blob/pubkeys/` | Yes | Re-fetchable from the mesh |
| `keys/`, `certs/`, `backup_vault/` | **No** | Key material, and never commit these |

The repository's `.rsyncignore` reflects this: `target`, `.git`, `image`, `nodeA`–`nodeN`,
`.env*`, `.ssh`, `*.log`. When copying the workspace, excluding the heavy generated trees
is what keeps it manageable:

```bash
rsync -av --exclude 'target' --exclude 'jobs' --exclude 'blobs' --exclude 'artifacts' \
  source/ destination/
```

## 6. Multi-node checklist

Running several nodes on one machine:

- [ ] One directory tree per node, created before launch
- [ ] `STORAGE_*` and `BLOB_STORAGE_DIR` exported separately in each shell
- [ ] No two nodes sharing a `midcid/`
- [ ] Distinct `listen_addr` ports AND Distinct `secret_seed` per node (identical seeds equal identical peer IDs) in the cluster TOML [mesh_a_cluster_local](../meshes_config/mesh_a_cluster_local.toml) 

---

*Build and run instructions: [GETTING_STARTED.md](GETTING_STARTED.md) · Storage model:
[ARCHITECTURE.md §4](ARCHITECTURE.md#4-how-storage-works)*
