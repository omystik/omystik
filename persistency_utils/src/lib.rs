use std::{error::Error, path::PathBuf};

use content_hashing::{newkey, encrypt_file, decrypt_file, remove_stored_file, get_storage_dir};
use metadata_mrkl::{Metadata, MonoMetaTree, hex_to_bytes, bytes_to_hex};



// ╔╗╔═╦═╦═╗╔╗
// ║╚╣╬║═╣╬╚╣╚╗
// ╚═╩═╩═╩══╩═╝
// Current Node - Store file
// return private key and nonce to decrypt file if needed + MID to provided to the network
pub async fn store_file(file_name: &str, file_type: &str, secret: &str, file_path: &str) -> Result<([u8; 32], [u8; 19], String, String), Box<dyn Error + Send + Sync>> {

    // create private key and nonce for file encryption/decryption
    let (private_key, nonce) = newkey().await?;

    // get the file's CID
    let cid_as_hex = encrypt_file(&private_key, &nonce, file_path).await?; //"../ghost-ground/data/mystikp2p.png"

    // init the metadata
    let mut metadata = Metadata::new(file_name.to_string(), file_type.to_string(), secret.to_string());

    // 1st level of Merkle (classic tree) get the metadata's MID
    let mid_as_hex = &metadata
        .create_tree(true)
        .await?;

    let mid_as_key  = hex_to_bytes::<32>(&mid_as_hex)
        .await?;
    
    // prooving that metadata exist    
    let leaves: &[&str] = &[file_name, file_type, secret];
    let meta_exist = metadata.prove_leaf_persistent(leaves).await?;

    if ! meta_exist{
        return Err("Given metadata are not stored in record".into());
    }

    // 2nd level of Merkle (Sparse tree)
    // init tree (Monotree) instance with the persistent database (rocksdb) and hasher (blake3)
    let mut tree = MonoMetaTree::new()?;

    // pairing MID as key and CID as leaf in insert 
    let cid_as_leaf = hex_to_bytes::<32>(&cid_as_hex).await?;
    tree.insert(&mid_as_key, &cid_as_leaf)?;

    // Verification of MID CID pair creation
    let midcid = tree.verify_merkle_proof(&mid_as_key, &cid_as_leaf)?;

    if !midcid {
        return Err("MID-CID pair not stored in record".into());
    }

    let hex_key = bytes_to_hex::<32>(private_key).await;
    println!("🔑 private_key: {:?}", hex_key);
    let hex_nonce = bytes_to_hex::<19>(nonce).await;
    print!("\n");
    println!("🔢 nonce: {:?}", hex_nonce);
    print!("\n");
    println!("mid_as_hex: {:?}", mid_as_hex);
    
    return  Ok((private_key, nonce, mid_as_hex.to_string(), cid_as_hex));
}


pub async  fn midkey_to_cidhex(mid_as_key: [u8;32])-> Result<String, Box<dyn Error + Send + Sync>> {
    // init tree (Monotree) instance with the persistent database (rocksdb) and hasher (blake3)
    let mut tree = MonoMetaTree::new()?;

    // get the MID CID pair
    if let Some(bytes_of_cid) = tree.get(&mid_as_key)?{
        let cid_as_hex = bytes_to_hex::<32>(bytes_of_cid).await;

        return Ok(cid_as_hex)
    };

    Ok("Wrong MID or No CID founded".into())
}

pub async  fn midhex_to_cidhex(mid_as_hex: &str)-> Result<String, Box<dyn Error + Send + Sync>> {
    // init tree (Monotree) instance with the persistent database (rocksdb) and hasher (blake3)

    let mid_as_key: [u8;32] = hex_to_bytes(mid_as_hex).await?;

    let mut tree = MonoMetaTree::new()?;

    // get the MID CID pair
    if let Some(bytes_of_cid) = tree.get(&mid_as_key)?{
        let cid_as_hex = bytes_to_hex::<32>(bytes_of_cid).await;

        return Ok(cid_as_hex)
    };

    Ok("Wrong MID or No CID founded".into())
}


// Current Node - Retrieve file
pub async fn get_cid_file(file_name: &str, file_type: &str, secret: &str)-> Result<String, Box<dyn Error + Send + Sync>> {

    // init the metadata
    let mut metadata = Metadata::new(file_name.to_string(), file_type.to_string(), secret.to_string());

    // prooving that metadata exist    
    let leaves: &[&str] = &[file_name, file_type, secret];
    let meta_exist = metadata.prove_leaf_persistent(leaves).await?;

    if ! meta_exist{
        return Err("Given metadata are not matching records".into());
    }

    // 1st level of Merkle (classic tree) get the metadata's MID
    let mid_as_key = hex_to_bytes::<32>(&metadata.create_tree(false).await?).await?;

    // 2nd level of Merkle (Sparse tree)
    // get the MID CID pair
    let cid_as_hex = midkey_to_cidhex(mid_as_key).await?;
       
    println!("cid test get_cid_file: {}", cid_as_hex);
    Ok(cid_as_hex)
} 

pub async fn get_decrypt_file(hex_key: &str, hex_nonce: &str, cid_as_hex: &str, destination_path: &str)-> Result<(), Box<dyn Error + Send + Sync>> { // "../content-hashing/data_dec/mystikp2p.png"
    let private_key: [u8; 32] = hex_to_bytes::<32>(hex_key).await?;
    let nonce: [u8; 19] = hex_to_bytes::<19>(hex_nonce).await?;
    decrypt_file(&private_key, &nonce, cid_as_hex, destination_path).await?;
    Ok(())
}

pub async fn remove_file(file_name: &str, file_type: &str, secret: &str)-> Result<bool, Box<dyn Error + Send + Sync>> {

    // init the metadata
    let mut metadata = Metadata::new(file_name.to_string(), file_type.to_string(), secret.to_string());

    // prooving that metadata exist    
    let leaves: &[&str] = &[file_name, file_type, secret];
    let meta_exist = metadata.prove_leaf_persistent(leaves).await?;

    if ! meta_exist{
        return Err("Given metadata are not matching records".into());
    }

    // 1st level of Merkle (classic tree) get the metadata's MID
    let mid_as_key = hex_to_bytes::<32>(&metadata.create_tree(false).await?).await?;

    // 2nd level of Merkle (Sparse tree)
    // init tree (Monotree) instance with the persistent database (rocksdb) and hasher (blake3)
    let mut tree = MonoMetaTree::new()?;

    // get the MID CID pair
    if let Some(bytes_of_cid) = tree
        .get(&mid_as_key)
        .expect("Failed to retrieved CID of content to delete"){

        let cid_as_hex = bytes_to_hex::<32>(bytes_of_cid).await;
        
        // remove encrypted stored file
        remove_stored_file(&cid_as_hex).await.expect("Critical error: Failed to remove encrypted stored of removed metadata. Stopping execution.");

        // remove metadata tree
        metadata.remove_record().await.expect("Critical error: Failed to remove metadata tree records. Stopping execution.");

        // remove MIDCID
        tree.remove(&mid_as_key).expect("Critical error: Failed to remove key. Stopping execution.");

        return Ok(true)
    };

    return Ok(false)
}

// TO DEL
pub async fn local_path_to_cid(mid_as_hex: &str)-> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    
    let cid_as_hex = midhex_to_cidhex(mid_as_hex).await?;
    let path = format!("{}{}.bin", get_storage_dir(), cid_as_hex).into();
    return Ok(path);
}

// ─────────╔╗
// ╔╦╦═╦══╦═╣╚╦═╗
// ║╔╣╩╣║║║╬║╔╣╩╣
// ╚╝╚═╩╩╩╩═╩═╩═╝

// Remote Node - Ask for file
pub async fn propose_mid_ntwk(file_name: &str, file_type: &str, secret: &str)-> Result<String, Box<dyn Error + Send + Sync>> {

    // init the metadata
    let mut metadata = Metadata::new(file_name.to_string(), file_type.to_string(), secret.to_string());

    // temporal creation  metadata's MID via 1st level of Merkle (classic tree) without persistent storing of MID
    let mid_as_key = hex_to_bytes::<32>(&metadata
        .transient_tree()
        .await?)
        .await?;

    let mid_as_hex = bytes_to_hex::<32>(mid_as_key).await;
   
    println!("mid_as_key propose_cid_ntwk: {:?}", mid_as_hex);
    Ok(mid_as_hex)
}

pub async fn create_persist_mid_ntwk(file_name: &str, file_type: &str, secret: &str)-> Result<String, Box<dyn Error + Send + Sync>> {

    // init the metadata
    let mut metadata = Metadata::new(file_name.to_string(), file_type.to_string(), secret.to_string());

    // 1st level of Merkle (classic tree) get the metadata's MID
    let mid_as_key = hex_to_bytes::<32>(&metadata.create_tree(true).await?).await?;

    // prooving that metadata exist    
    let leaves: &[&str] = &[file_name, file_type, secret];
    let meta_exist = metadata.prove_leaf_persistent(leaves).await?;

    if ! meta_exist{
        return Err("Given metadata are not matching records".into());
    }

    let mid_as_hex = bytes_to_hex::<32>(mid_as_key).await;
   
    println!("Persistent MID: {:?} created", mid_as_hex);
    Ok(mid_as_hex)
}

pub async fn import_ntwk_file(mid_as_hex: &str, cid_as_hex: &str) -> Result<(), Box<dyn Error + Send + Sync>> {

    let mid_as_key  = hex_to_bytes::<32>(&mid_as_hex)
        .await?;
    
    // 2nd level of Merkle (Sparse tree)
    // init tree (Monotree) instance with the persistent database (rocksdb) and hasher (blake3)
    let mut tree = MonoMetaTree::new()?;

    // pairing MID as key and CID as leaf in insert 
    let cid_as_leaf = hex_to_bytes::<32>(&cid_as_hex).await?;
    tree.insert(&mid_as_key, &cid_as_leaf)?;

    // Verification of MID CID pair creation
    let midcid = tree.verify_merkle_proof(&mid_as_key, &cid_as_leaf)?;

    if !midcid {
        return Err("MID-CID pair not stored in record".into());
    }

    let saving_confirmation = format!("Imported file well saved:\nMID: {}\nCID: {}", mid_as_hex, cid_as_hex);
    print!("{}", saving_confirmation);

    return  Ok(());
}