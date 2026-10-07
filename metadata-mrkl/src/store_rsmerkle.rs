use rs_merkle::{self, algorithms::Sha256, MerkleTree};
use std::error::Error;

use std::{fs, path::Path};

//const STORAGE_METADATA: &str = "./data/metadata/metamerkle/";
pub fn get_storage_metadata() -> String {
    std::env::var("STORAGE_METADATA").unwrap_or_else(|_| "./data/metadata/metamerkle/".to_string())
}

pub struct RSMerkleTree{
    leaves: Vec<[u8; 32]>,
    rs_tree: MerkleTree<Sha256>,
}

impl RSMerkleTree {

    // initiate a Merkle Tree
    pub async fn new(leaves: Vec<[u8; 32]>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let rs_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        Ok(Self { leaves, rs_tree })
    }

    // return tree
    pub fn inner_tree(self) -> MerkleTree<Sha256> {
        self.rs_tree
    }

    // temporal root creation (persistent case is handled by PersistentRsMerkle Tree)
    pub async fn temporal_root_tree(&mut self) -> Result<String, Box<dyn Error + Send + Sync>> {

        // tree not committed then root not permanent
        match self.rs_tree.root_hex() {
            Some(temp_root) => Ok(temp_root),
            None => Err("Root not available".into()),
        }
    }
}


pub struct PersistentRsMerkle {
    pub storage_path: String,
    pub tree: Option<MerkleTree<Sha256>>
}

impl PersistentRsMerkle {
    
    pub async fn new(root_hex: Option<&str>, tree: Option<MerkleTree<Sha256>>) -> Result<Self, Box<dyn Error + Send + Sync>>{
        
        let storage_roothex = if let Some(ref tree) = tree {
           
            format!("{}{}/", get_storage_metadata(), tree.root_hex().ok_or("couldn't get the merkle root")?)
        } else {
            format!("{}{}/", get_storage_metadata(), root_hex.unwrap_or("default_root_hex"))
        };

        fs::create_dir_all(&storage_roothex)?;

        Ok (Self {
            storage_path: storage_roothex,
            tree,
            })
    }

    // store the root
    pub async fn store_root(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>{
        
        let db_path = format!("{}/root.bin", self.storage_path);
        
        if let Some(ref mut tree) = self.tree{
            tree.commit();
        
            if let Some(root) = tree.root() {
            //let data = bincode::serialize(&root).expect("Failed to serialize bincode root");
                fs::write(db_path, &root)?;//.expect}("Failed to write serialize root");
            }
        }

        Ok(())
    }

    // store the tree structure
    pub async fn store_tree(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>{
        
        let db_path = format!("{}/tree.bin", self.storage_path);

        if let Some(ref mut tree)= self.tree {
            tree.commit();
            
            let leaves = tree.leaves();

            let flattened_leaves: Vec<u8> = leaves
                .iter()
                .flat_map(|leaf| leaf.iter().copied())
                .flatten()
                .collect();

            fs::write(db_path, &flattened_leaves)?; //.expect("Failed to write serialize tree");
        }

        Ok(())
    }

    pub async fn load_tree(&mut self) -> Result<Self, Box<dyn Error + Send + Sync>> {

        let db_path = format!("{}/tree.bin", self.storage_path);

        if !Path::new(&db_path).exists() {
            return Err(format!("Tree file not found at {}", db_path).into());
        }

        let data = fs::read(db_path)?;

        // Ensure data is a multiple of 32 bytes (valid `[u8; 32]` leaves)
        if data.len() % 32 != 0 {
            return Err("Invalid tree file size, not a multiple of 32".into());
        }

        let leaves: Vec<[u8; 32]> = data
            .chunks_exact(32)
            .map(|chunk| chunk.try_into().expect("Failed to convert chunk to [u8;32]"))
            .collect();
    
        let loaded_tree = MerkleTree::<Sha256>::from_leaves(&leaves);

        Ok (Self {
            storage_path: self.storage_path.clone(),
            tree: Some(loaded_tree),
        })
    
    }

    /// Verifies if the stored root matches the reconstructed root
    pub async fn verify_root_tree(&mut self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        
        let root_path = format!("{}/root.bin", self.storage_path);

        if let Some(ref mut tree)= self.tree {
            tree.commit();
        
            if Path::new(&root_path).exists() {
                let stored_root = fs::read(root_path)?;

               if stored_root.len() != 32 {
                return Err(format!("Incorrect root size: {:?}", stored_root).into());
            }

            let stored_root: [u8; 32] = stored_root.try_into().map_err(|_| "Failed to convert stored root to [u8; 32]")?;
            let computed_root = tree.root().ok_or("No root found in tree")?;
                return Ok(stored_root == computed_root);
            }
        }
        return Ok(false)
        
    }

    // Del tree, root and leafs of an obsolete record
    pub async fn delete_merkle_meta(&mut self) -> Result<bool, Box<dyn Error + Send + Sync>> {
        
        println!("deleted check{}", self.storage_path);

        if fs::metadata(&self.storage_path).is_ok(){
            fs::remove_dir_all(&self.storage_path)?; // delete the content of folder

            println!("{} metadata tree-folder deleted successfully", self.storage_path);
            return Ok(true);
        }

        return Ok(false)
    }

}

// fn main() {

//     let file_cid = "f514drs909f4g9z4w"; // The actual file path
//     let mut metadata_tree = PersistentMetaMerkle::new(file_cid);

//     // Example metadata (3 values per file)
//     let mut metadata_entries = vec![
//         Sha256::hash(b"FROMABOVE"), 
//         Sha256::hash(b"mp4"), 
//         Sha256::hash(b"BXO545")
//     ];

//     // Save each metadata entry
//     for (i, meta) in metadata_entries.iter().enumerate() {
//         metadata_tree.store_leaf(i, meta);
//     }

//     // Add metadata to Merkle tree
//     metadata_tree.tree.append(&mut metadata_entries);

//     metadata_tree.tree.commit();

//     // Save the tree and its root
//     metadata_tree.store_tree();
//     metadata_tree.store_root();

//     println!("Metadata stored in folder '{}'", metadata_tree.storage_path);

    
// }
