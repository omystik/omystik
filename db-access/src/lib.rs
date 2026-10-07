use tracing; 
use std::error::Error;

use std::fs;
use std::path::{Path, PathBuf};

use content_hashing::get_storage_dir;
use metadata_mrkl::get_storage_merkle;


/// Reconstructs MID/CID pairs by examining the file system directly.
/// CIDs are stored as .bin files in content directory
/// MIDs are represented as subfolder names in metadata/metamerkle directory
pub async fn get_midcid_pairs() -> Result<Vec<(String, String)>, Box<dyn Error + Send + Sync>> {

    // Get content directory by calling get_storage_dir()
    let content_dir = get_storage_dir();
    let metadata_dir = get_storage_merkle();

    println!("content_dir: {}", content_dir);
    println!("metadata_dir: {}", metadata_dir);

    // Get all CID files from content directory
    let mut cid_paths = Vec::new();
    visit_content_files(Path::new(&content_dir), &mut cid_paths)?;
    // tracing::info!("Found {} CID files", cid_paths.len());
    
    // Get all MID directories
    let mid_dirs = fs::read_dir(&metadata_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<String>>();
    // tracing::info!("Found {} MID directories", mid_dirs.len());
    
    // For each CID, read its metadata to find the corresponding MID
    let mut pairs = Vec::new();
    for cid_path in &cid_paths {
        let file_stem = cid_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        
        let cid = file_stem.to_string();
        
        // Read the file metadata to determine the corresponding MID
        // This is a simplified approach - you might need more complex logic
        // depending on how the files are actually connected
        for mid in &mid_dirs {
            let potential_link_path = Path::new(&metadata_dir)
                .join(mid)
                .join(format!("{}.link", cid));
            
            if potential_link_path.exists() {
                pairs.push((mid.clone(), cid.clone()));
                // tracing::info!("Found pair: {} -> {}", mid, cid);
                break;
            }
        }
    }
    
    if pairs.is_empty() {
        // Fallback: try to match by timestamps or other metadata
        match_by_timestamps(&cid_paths, &mid_dirs, &mut pairs)?;
    }
    
    // tracing::info!("Reconstructed {} MID/CID pairs", pairs.len());
    
    if pairs.is_empty() {
        // tracing::debug!("No MID/CID pairs could be reconstructed");
        Ok(Vec::new())
    } else {
        Ok(pairs)
    }
}

/// Recursively visits content files (.bin) and adds their paths to the result vector
fn visit_content_files(dir: &Path, result: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error + Send + Sync>> {
    if dir.is_dir() {

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                visit_content_files(&path, result)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                result.push(path);
            }
        }
    }
    Ok(())
}

/// Attempts to match MIDs and CIDs based on file timestamps or other metadata
fn match_by_timestamps(
    cid_paths: &[PathBuf], 
    mid_dirs: &[String],
    pairs: &mut Vec<(String, String)>
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Get metadata for all CID files
    let mut cid_metadata: Vec<_> = cid_paths.iter()
        .filter_map(|path| {
            let metadata = fs::metadata(path).ok()?;
            let created = metadata.created().ok()?;
            let file_stem = path.file_stem()?.to_str()?;
            Some((file_stem.to_string(), created))
        })
        .collect();
    
    // Get metadata for all MID directories
    let mut mid_metadata: Vec<_> = mid_dirs.iter()
        .filter_map(|mid| {
            let dir_path = Path::new(&get_storage_merkle()).join(mid);
            let metadata = fs::metadata(&dir_path).ok()?;
            let created = metadata.created().ok()?;
            Some((mid.clone(), created))
        })
        .collect();
    
    // Sort both by timestamp
    cid_metadata.sort_by(|a, b| a.1.cmp(&b.1));
    mid_metadata.sort_by(|a, b| a.1.cmp(&b.1));
    
    // // tracing::info!("Sorted CIDs by timestamp:");
    // for (cid, time) in &cid_metadata {
    //     tracing::info!("CID: {} at {:?}", cid, time);
    // }
    
    // // tracing::info!("Sorted MIDs by timestamp:");
    // for (mid, time) in &mid_metadata {
    //     tracing::info!("MID: {} at {:?}", mid, time);
    // }
    
    // Match them in sorted order
    let min_len = std::cmp::min(cid_metadata.len(), mid_metadata.len());
    for i in 0..min_len {
        let (mid, mid_time) = &mid_metadata[i];
        let (cid, cid_time) = &cid_metadata[i];
        pairs.push((mid.clone(), cid.clone()));
        tracing::info!("Matched by timestamp: {} ({:?}) -> {} ({:?})", mid, mid_time, cid, cid_time);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_get_midcid_pairs() {
        let pairs = get_midcid_pairs().await.unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            Vec::new()
        });
        
        assert!(!pairs.is_empty(), "Should find at least one MID/CID pair");
        
        for (mid, cid) in &pairs {
            println!("MID: {} -> CID: {}", mid, cid);
        }
    }
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
//    
//     Ok(())
// }
