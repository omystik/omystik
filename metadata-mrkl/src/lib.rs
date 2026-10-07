use std::{error::Error, result::Result};

use rs_merkle::{self, algorithms::Sha256, Hasher as MerkleHasher, MerkleTree, MerkleProof};

use hex::{encode, decode};

use monotree::{
    database::sled::Sled, hasher::{Blake3, Hasher}, verify_proof,Errors, Monotree, Result as MonoTResult
};

use std::convert::TryInto;

mod store_rsmerkle;
use store_rsmerkle::{RSMerkleTree, PersistentRsMerkle};

//pub const STORAGE_MERKLE: &str = "./data/metadata/midcid/";
pub fn get_storage_merkle() -> String {
    std::env::var("STORAGE_MERKLE").unwrap_or_else(|_| "./data/metadata/metamerkle/".to_string())
}

//pub const STORAGE_MIDCID: &str = "./data/metadata/midcid/";
pub fn get_storage_midcid() -> String {
    std::env::var("STORAGE_MIDCID").unwrap_or_else(|_| "./data/metadata/midcid/".to_string())
}

pub struct Metadata {
    file_name: String,
    file_type: String,
    secret: String,
}

impl Metadata {
    pub fn new(
        file_name: String,
        file_type: String,
        secret: String,
    ) -> Self {
        
        Self {
            file_name,
            file_type,
            secret,
    }
}
    // Metadata Tree creation
    pub async fn create_tree(&mut self, store_tree: bool) -> Result<String, Box<dyn Error + Send + Sync>> {
        
        let leaves = self.leaf_basis_hashing().await?;

        let merkletree = RSMerkleTree::new(leaves.clone())
            .await?
            .inner_tree();
        
        let mut persistentstorage = PersistentRsMerkle::new(None, Some(merkletree)).await?;

         // Store tree, and root if needed
        if store_tree {
            persistentstorage.store_tree().await?;
            persistentstorage.store_root().await?;
        }
    
        let root_hex = persistentstorage
            .tree
            .as_ref()
            .ok_or("Merkle tree not found")?
            .root_hex()
            .ok_or("Could not retrieve Merkle root")?;
        
        Ok(root_hex)
    }

    // Metadata Tree Transient Creation TO DEL
    pub async fn transient_tree(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {
        
        let leaves = self.leaf_basis_hashing().await?;

        let mut merkletree = RSMerkleTree::new(leaves.clone()).await?;

        let root_hex = merkletree.temporal_root_tree().await?;
        
        println!("Transcient MID: {}", root_hex);
        Ok(root_hex)
    }


    // Leaves hashing
    async fn leaf_basis_hashing(&self) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let metadata = [&self.file_name, &self.file_type, &self.secret];
        let leaves: Vec<[u8; 32]> = metadata
            .iter()
            .map(|x| Sha256::hash(x.as_bytes()))
            .collect();
        Ok(leaves)
    }

    // Retrieve a root
    async fn get_root_hex(&self) -> Result<String, Box<dyn Error + Send + Sync>> {
        
        let leaves: Vec<[u8; 32]> = self.leaf_basis_hashing().await?;

        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        // let current_root_hex = merkle_tree.root_hex().ok_or("Not able to return current root hex")?;
        // println!("current root hex (internal): {}", current_root_hex);
        let mut persistent_meta_merkle = PersistentRsMerkle::new(None, Some(merkle_tree)).await?;
        let is_valid = persistent_meta_merkle.verify_root_tree().await?;

        if ! is_valid {
            return Err("Unvalid: stored tree for provided leaves.".into())
        }
        
        let root_hex = persistent_meta_merkle
            .tree
            .as_ref()
            .ok_or("Merkle tree not found")?
            .root_hex()
            .ok_or("Could not retrieve Merkle root")?;
        
        Ok(root_hex)
    }

    // Remove a record
    pub async fn remove_record(&self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        
        let roothex_retrieved = self.get_root_hex().await?;
        let mut persistent_rsmerkle = PersistentRsMerkle::new(Some(&roothex_retrieved), None).await?;
        
        // Delete current tree folder.
        let is_deleted = persistent_rsmerkle.delete_merkle_meta().await?;

        if ! is_deleted {
            return Ok(false);
        }

        return Ok(true);
    }


    // Proof Leaf
    pub async fn prove_leaf_persistent(&self, initial_leaves: &[&str]) -> Result<bool, Box<dyn Error + Send + Sync>> {
        
        let roothex_retrieved = self.get_root_hex().await?;
        let mut persistent_rsmerkle = PersistentRsMerkle::new(Some(&roothex_retrieved), None).await?;
        let instant_tree = persistent_rsmerkle
            .load_tree()
            .await?
            .tree
            .expect("Tree is None");

        let leaves: Vec<[u8; 32]> = initial_leaves
            .iter()
            .map(|x| Sha256::hash(x.as_bytes()))
            .collect();

        let indices_to_prove: Vec<usize> = (0..leaves.len()).collect();

        let merkle_proof = instant_tree.proof(&indices_to_prove);
        let merkle_root = instant_tree.root().ok_or("can't get current root")?;

        // serialize proof to pass it to the client
        let proof_bytes = merkle_proof.to_bytes();

        // Parse proof back on the client
        let proof = MerkleProof::<Sha256>::try_from(proof_bytes)?;

        let leave_verify = proof.verify(merkle_root, &indices_to_prove, &leaves, leaves.len());
        Ok(leave_verify)
    }

}


pub struct MonoMetaTree {
    pub tree: Monotree<Sled, Blake3>,
    root: Option<[u8; 32]>,
}

impl MonoMetaTree {

    // init the Monotree instance (database > tree and root)
    pub fn new() -> MonoTResult<Self>{

        let storage_midcid = format!("{}", get_storage_midcid());

        let mut newtree = Monotree::<Sled, Blake3>::new(&storage_midcid);
        let head_root = newtree.get_headroot()?;
        
        Ok(MonoMetaTree{
            tree: newtree,
            root: head_root,
        })
    }

    // insert key-leaf pair into the tree
    pub fn insert(&mut self, key: &[u8; 32], leaf: &[u8; 32]) -> MonoTResult<()> {
        self.root = self.tree.insert(self.root.as_ref(), &key, &leaf)?;
        self.tree.commit();
        self.tree.set_headroot(self.root.as_ref());

        Ok(())
    }

    // remove en entry (aka a key) from the tree
    pub fn remove(&mut self, key: &[u8]) -> MonoTResult<()> {
        self.root = self.tree.remove(self.root.as_ref(), &key)?;
        self.tree.commit();
        self.tree.set_headroot(self.root.as_ref());

        Ok(())
    }

    // get (or find) a leaf or a leaf for a given leaf from the tree
    pub fn get(&mut self, key: &[u8; 32]) -> MonoTResult<Option<[u8; 32]>> {
        let root = self.tree.get_headroot()?;
        match self.tree.get(root.as_ref(), &key)? {
            Some(leaf) => Ok(Some(leaf)),
            None => Err(Errors::new(&format!("Failed to get leaf{:?}", key))),
        }
    }

    // generate the Merkle proof for the root and the key
    pub fn proof(&mut self, key: &[u8; 32]) -> MonoTResult<Option<Vec<(bool, Vec<u8>)>>> {
        let root = self.tree.get_headroot()?;
        let proof = self.tree.get_merkle_proof(root.as_ref(), key)?;
        return Ok(proof)
    }

    // generate the Merkle proof for the root and the key
    pub fn verify_merkle_proof(&mut self, key: &[u8; 32], leaf: &[u8; 32]) -> MonoTResult<bool> {
        let protocol_hash = Blake3::new();
        let proved_key = self.proof(key)?;
        let verified = verify_proof(&protocol_hash, self.root.as_ref(), &leaf, proved_key.as_ref());
        Ok(verified)
    }

}

/// String to Hex conversion (CID and MID essentially)

/// Converts a HEX string into a fixed-size byte array of size `N`.
pub async fn hex_to_bytes<const N: usize>(hex_str: &str) -> Result<[u8; N], Box<dyn Error + Send + Sync>> {
    let decoded_bytes = decode(hex_str)?;
    let byte_array: [u8; N] = decoded_bytes
        .try_into()
        .map_err(|_| format!("Hex string is not {} bytes long", N))?;
    Ok(byte_array)
}

/// Converts a fixed-size byte array of size `N` back into a HEX string.
pub async fn bytes_to_hex<const N: usize>(bytes: [u8; N]) -> String {
    encode(bytes)
}

///// TESTING SECTION
// async fn newkey() -> Result<([u8; 32], [u8; 19]), Box <dyn Error>> {

//     let mut key = [0u8; 32];
//     let mut nonce = [0u8; 19];

//     OsRng.fill_bytes(&mut key);
//     OsRng.fill_bytes(&mut nonce);
    
//     Ok((key, nonce))
// }

// Generates a CID from content using Blake3
// async fn generate_cid(content: &str) -> String {
//     let mut hasher = GenuineBk3::new();
//     hasher.update(content.as_bytes());
//     hasher.finalize().to_hex().to_string() // Returns a hex-encoded CID string
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    
//     let mut empty_leaves: Vec<&str>= Vec::new();
    
//     let file_name = "ObserveThis".to_string();
//     empty_leaves.push(&file_name);
//     let file_type = "txt".to_string();
//     empty_leaves.push(&file_type);
//     let secret = "BCX451".to_string();
//     empty_leaves.push(&secret);


//     let cid = generate_cid(&file_name.clone()).await;
//     empty_leaves.push(&cid);

//     let initial_leaves: &[&str] = &empty_leaves;
    
//     let mut metadata = Metadata::new(file_name.clone(), file_type.clone(), secret.clone(), cid.clone());
//     let roothex_aka_mid = cid_to_bytes(&metadata.create_tree().await?).await?;


//     //let key = monotree::utils::random_hash();//metadata.key_hashing();
//     //println!("Key:\n{:?}", key);
//     // init Monotree instance with the persistent database (rocksdb) and hasher (blake3)
//     let mut tree = MonoMetaTree::new()?;

//     let leaf_aka_cid = cid_to_bytes(&metadata.cid).await?;
//     tree.insert(&roothex_aka_mid, &leaf_aka_cid)?;

//     // reverse the peedID
//     let cid_recomposed = bytes_to_cid(leaf_aka_cid).await;
    
//     if cid == cid_recomposed {
//         print!("Banger\n");
//     } else {
//         print!("Schwarz\n");
//     }

   
//     let leaf_proof = metadata.prove_leaf_persistent(initial_leaves).await;
//     println!("Leaf Proof: {:?}", leaf_proof);
//     Ok(())
// }