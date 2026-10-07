
--- Local Monotree

1. *** Metadata / file's CID pairing ***
Each node that stores files maintains a local monotree (Sparse Merkle Tree).
For each file stored, the node calculates key = hash(metadata) and stores (key, cid) in its local monotree. That yields or updates a local Merkle root for that node.


2. *** DHT Advertisement ***
Each node then advertises in the DHT.
Typically, this is done by putting an entry in the DHT that maps key => peer_id.
Alternatively, you might store (key => (peer_id, merkle_root)) if you also want to retrieve a cryptographic proof from the node’s monotree.
File Lookup:

A user on another node wants the file associated with some (file_name, file_type, mission).
They compute the same key = hash(metadata).
They do a DHT query: “Which node(s) store key?”
The DHT returns a list of peer_ids that claim to store (key => cid).
Retrieval:

The user’s node connects to one of those peer nodes.
It requests the CID. Optionally, it might also request a Merkle proof of (key => cid) from that peer’s local monotree so it can verify tamper-evidence.
Once it trusts or accepts the response, it obtains the cid and can fetch the encrypted file via the DHT (or direct p2p transfer) from that peer.
This design ensures:

Privacy of metadata: The actual (file_name, file_type, mission) are never published in plaintext. Only hash(metadata) is stored in the local monotree and published as a key in the DHT.
Local Control: Each node manages its own monotree for the files it stores, so there is no single “global root” problem.
Discovery: The DHT maps hash(metadata) to whichever node has that file.
Proof (if desired): The user can ask for a Sparse Merkle proof from the storing node’s monotree to confirm the (key => cid) mapping is correct (as committed in that node’s local Merkle root).
