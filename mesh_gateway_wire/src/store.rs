use anyhow::{Context, Result};
use libp2p::PeerId;
use crate::{
    BlobId, JobId, JobMetadata, JobState, JobType, Payload,
    gateway::JobResultPayload,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasRecord {
    pub alias: crate::ArtifactAlias,
    pub manifest_cid: BlobId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: JobId,
    pub job_type: JobType,
    pub payload: Payload,
    pub metadata: JobMetadata,

    pub state: JobState,
    pub attempt: u32,
    pub last_error: Option<String>,

    pub result: Option<JobResultPayload>,
}

#[derive(Debug, Clone)]
pub struct JobStore {
    jobs_dir: PathBuf,
    blobs_dir: PathBuf,
    acks_dir: PathBuf,
    programs_dir: PathBuf,
    program_refs_dir: PathBuf,
    artifact_aliases_dir: PathBuf,
    artifact_manifests_dir: PathBuf,
}

impl JobStore {
    pub async fn open(data_dir: PathBuf) -> Result<Self> {
        let jobs_dir = data_dir.join("jobs");
        let blobs_dir = data_dir.join("blobs");
        let acks_dir = data_dir.join("acks");
        let programs_dir = data_dir.join("programs");
        let program_refs_dir = data_dir.join("program_refs");
        let artifact_aliases_dir = data_dir.join("artifact_aliases");
        let artifact_manifests_dir = data_dir.join("artifact_manifests");

        fs::create_dir_all(&jobs_dir).await?;
        fs::create_dir_all(&blobs_dir).await?;
        fs::create_dir_all(&acks_dir).await?;
        fs::create_dir_all(&programs_dir).await?;
        fs::create_dir_all(&program_refs_dir).await?;
        fs::create_dir_all(&artifact_aliases_dir).await?;
        fs::create_dir_all(&artifact_manifests_dir).await?;

        Ok(Self {
            jobs_dir,
            blobs_dir,
            acks_dir,
            programs_dir,
            program_refs_dir,
            artifact_aliases_dir,
            artifact_manifests_dir,
        })
    }

    pub fn job_path(&self, job_id: &JobId) -> PathBuf {
        self.jobs_dir.join(format!("{}.json", hex32(*job_id)))
    }

    pub fn blob_path(&self, blob_id: &BlobId) -> PathBuf {
        self.blobs_dir.join(format!("{}.bin", hex32(*blob_id)))
    }

    pub fn ack_path(&self, key_id: &[u8; 32]) -> PathBuf {
        self.acks_dir.join(format!("{}.json", hex32(*key_id)))
    }

    /// Record a replication ack from a Mesh A peer for a given (key_id, blob_id).
    pub async fn record_pin_ack(
        &self,
        key_id: &[u8; 32],
        blob_id: &BlobId,
        provider: PeerId,
    ) -> Result<usize> {
        #[derive(Serialize, Deserialize, Default)]
        struct AckRec {
            blob_id: BlobId,
            providers: Vec<String>,
        }

        let path = self.ack_path(key_id);

        let mut rec: AckRec = match fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AckRec::default(),
            Err(e) => return Err(e.into()),
        };

        if rec.providers.is_empty() {
            rec.blob_id = *blob_id;
        } else if rec.blob_id != *blob_id {
            return Err(anyhow::anyhow!("ack blob_id mismatch for key_id"));
        }

        let provider_str = provider.to_string();
        if !rec.providers.iter().any(|p| p == &provider_str) {
            rec.providers.push(provider_str);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let bytes = serde_json::to_vec_pretty(&rec)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).await?;
        fs::rename(&tmp, &path).await?;

        Ok(rec.providers.len())
    }

    pub async fn ack_count(&self, key_id: &[u8; 32]) -> Result<usize> {
        #[derive(Deserialize)]
        struct AckRec {
            providers: Vec<String>,
        }

        let path = self.ack_path(key_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let rec: AckRec = serde_json::from_slice(&bytes)?;
                Ok(rec.providers.len())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    pub fn signed_manifest_path(&self, manifest_cid: &BlobId) -> PathBuf {
        self.artifact_manifests_dir
            .join(format!("{}.json", hex32(*manifest_cid)))
    }

    pub async fn set_signed_manifest(
        &self,
        manifest_cid: &BlobId,
        manifest: &crate::SignedManifest,
    ) -> Result<()> {
        let path = self.signed_manifest_path(manifest_cid);
        let bytes = serde_json::to_vec_pretty(manifest)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn get_signed_manifest(
        &self,
        manifest_cid: &BlobId,
    ) -> Result<Option<crate::SignedManifest>> {
        let path = self.signed_manifest_path(manifest_cid);
        match fs::read(path).await {
            Ok(bytes) => {
                let rec: crate::SignedManifest = serde_json::from_slice(&bytes)?;
                Ok(Some(rec))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Resolve a signed manifest by logical artifact-set id.
    /// This scans canonical artifact manifests and returns the first one whose inner manifest.id matches `id`.
    /// If you later add versioning/current-selection logic, centralize it here.
    pub async fn get_manifest_by_id(
        &self,
        id: &[u8; 32],
    ) -> Result<Option<crate::SignedManifest>> {
        let mut rd = match fs::read_dir(&self.artifact_manifests_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let mut best: Option<crate::SignedManifest> = None;

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let bytes = fs::read(&path).await?;
            let signed: crate::SignedManifest = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse signed manifest {:?}", path))?;

            if signed.manifest.id != *id {
                continue;
            }

            let should_replace = match &best {
                None => true,
                Some(current) => {
                    let new_entry_count = signed.manifest.entries.len();
                    let old_entry_count = current.manifest.entries.len();

                    new_entry_count > old_entry_count
                        || (new_entry_count == old_entry_count
                            && signed.manifest.version > current.manifest.version)
                }
            };

            if should_replace {
                best = Some(signed);
            }
        }

        Ok(best)
    }

    /// Upsert an artifact entry inside the canonical manifest identified by logical `id`.
    ///
    /// This preserves the current API used by runtime code, but writes only to the
    /// canonical signed-manifest store under `artifact_manifests/`.
    ///
    /// If a matching manifest already exists for `id`, it is updated in place and, if
    /// its content changes, re-written under its new content-derived CID.
    pub async fn set_artifact(
        &self,
        id: &[u8; 32],
        kind: crate::ArtifactKind,
        blob_id: &BlobId,
    ) -> Result<()> {
        let (mut entries, metadata) = self.collect_manifest_entries_for_id(id).await?;

        let byte_len = match self.read_blob(blob_id).await {
            Ok(bytes) => bytes.len() as u64,
            Err(_) => 0,
        };

        let new_entry = crate::ArtifactEntry {
            kind,
            cid: *blob_id,
            byte_len,
            media_type: "application/octet-stream".into(),
        };

        if let Some(existing) = entries.iter_mut().find(|e| e.kind == new_entry.kind) {
            *existing = new_entry;
        } else {
            entries.push(new_entry);
        }

        let signed = crate::SignedManifest {
            manifest: crate::ArtifactManifest {
                id: *id,
                version: 1,
                entries,
                metadata,
            },
            signatures: Vec::new(),
        };

        let new_manifest_cid = crate::blake3_blob_id(&crate::encode(&signed)?);

        self.set_signed_manifest(&new_manifest_cid, &signed).await?;
        self.remove_manifests_for_id_except(id, &new_manifest_cid).await?;

        Ok(())
    }
    
    async fn collect_manifest_entries_for_id(
        &self,
        id: &[u8; 32],
    ) -> Result<(Vec<crate::ArtifactEntry>, Vec<(String, String)>)> {
        let mut rd = match fs::read_dir(&self.artifact_manifests_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Vec::new(), Vec::new()));
            }
            Err(e) => return Err(e.into()),
        };

        let mut entries: Vec<crate::ArtifactEntry> = Vec::new();
        let mut metadata: Vec<(String, String)> = Vec::new();

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let Ok(signed) = serde_json::from_slice::<crate::SignedManifest>(&bytes) else {
                continue;
            };

            if signed.manifest.id != *id {
                continue;
            }

            if metadata.is_empty() {
                metadata = signed.manifest.metadata;
            }

            for entry in signed.manifest.entries {
                if let Some(existing) = entries.iter_mut().find(|e| e.kind == entry.kind) {
                    *existing = entry;
                } else {
                    entries.push(entry);
                }
            }
        }

        Ok((entries, metadata))
    }

    async fn remove_manifests_for_id_except(
        &self,
        id: &[u8; 32],
        keep_cid: &BlobId,
    ) -> Result<()> {
        let keep_path = self.signed_manifest_path(keep_cid);

        let mut rd = match fs::read_dir(&self.artifact_manifests_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();

            if path == keep_path {
                continue;
            }

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };

            let Ok(signed) = serde_json::from_slice::<crate::SignedManifest>(&bytes) else {
                continue;
            };

            if signed.manifest.id == *id {
                let _ = fs::remove_file(&path).await;
            }
        }

        Ok(())
    }

    pub async fn get_artifact(
        &self,
        id: &[u8; 32],
        kind: crate::ArtifactKind,
    ) -> Result<Option<BlobId>> {
        let mut rd = match fs::read_dir(&self.artifact_manifests_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let mut best: Option<(usize, BlobId)> = None;

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let bytes = fs::read(&path).await?;
            let signed: crate::SignedManifest = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse signed manifest {:?}", path))?;

            if signed.manifest.id != *id {
                continue;
            }

            let Some(entry) = signed.manifest.entries.iter().find(|e| e.kind == kind) else {
                continue;
            };

            let entry_count = signed.manifest.entries.len();

            let replace = match best {
                None => true,
                Some((best_count, _)) => entry_count > best_count,
            };

            if replace {
                best = Some((entry_count, entry.cid));
            }
        }

        Ok(best.map(|(_, cid)| cid))
    }

    pub async fn load_job(&self, job_id: &JobId) -> Result<Option<JobRecord>> {
        let path = self.job_path(job_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let rec: JobRecord = serde_json::from_slice(&bytes)
                    .with_context(|| format!("parse job record {:?}", path))?;
                Ok(Some(rec))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("read job record {:?}", path)),
        }
    }

    pub async fn save_job(&self, rec: &JobRecord) -> Result<()> {
        let path = self.job_path(&rec.job_id);
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(rec)?;
        fs::write(&tmp, &bytes).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn blob_exists(&self, blob_id: &BlobId) -> bool {
        fs::metadata(self.blob_path(blob_id)).await.is_ok()
    }

    pub async fn write_blob(&self, blob_id: &BlobId, bytes: &[u8]) -> Result<()> {
        let path = self.blob_path(blob_id);
        let tmp = path.with_extension("bin.tmp");
        fs::write(&tmp, bytes).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn read_blob(&self, blob_id: &BlobId) -> Result<Vec<u8>> {
        let path = self.blob_path(blob_id);
        let bytes = fs::read(&path).await.with_context(|| format!("read blob {:?}", path))?;
        Ok(bytes)
    }

    pub fn blobs_dir(&self) -> &Path {
        &self.blobs_dir
    }

    pub async fn list(&self) -> Result<Vec<JobRecord>> {
        let mut out = Vec::new();
        let mut rd = fs::read_dir(&self.jobs_dir).await?;

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).await?;
            let rec: JobRecord = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse job record {:?}", path))?;
            out.push(rec);
        }
        Ok(out)
    }

    pub async fn update_state<F>(&self, job_id: &JobId, f: F) -> Result<()>
    where
        F: FnOnce(&mut JobRecord),
    {
        let mut rec = self
            .load_job(job_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;

        f(&mut rec);
        self.save_job(&rec).await?;
        Ok(())
    }

    pub fn installed_program_path(&self, package_blob_id: &BlobId) -> PathBuf {
        self.programs_dir.join(format!("{}.json", hex32(*package_blob_id)))
    }

    pub fn installed_program_ref_path(
        &self,
        program_id: &compute_abi::ProgramId,
        program_version: &compute_abi::ProgramVersion,
    ) -> PathBuf {
        self.program_refs_dir
            .join(format!("{}__{}.json", sanitize(&program_id.0), sanitize(&program_version.0)))
    }

    pub async fn install_program(
        &self,
        record: &compute_abi::InstalledProgramRecord,
    ) -> Result<()> {
        let record_path = self.installed_program_path(&record.package_blob_id);
        let ref_path = self.installed_program_ref_path(&record.program_id, &record.program_version);

        let record_bytes = serde_json::to_vec_pretty(record)?;
        let ref_bytes = serde_json::to_vec_pretty(record)?;

        let record_tmp = record_path.with_extension("json.tmp");
        let ref_tmp = ref_path.with_extension("json.tmp");

        fs::write(&record_tmp, &record_bytes).await?;
        fs::rename(&record_tmp, &record_path).await?;

        fs::write(&ref_tmp, &ref_bytes).await?;
        fs::rename(&ref_tmp, &ref_path).await?;

        Ok(())
    }

    pub async fn get_installed_program(
        &self,
        program_id: &compute_abi::ProgramId,
        program_version: &compute_abi::ProgramVersion,
    ) -> Result<Option<compute_abi::InstalledProgramRecord>> {
        let path = self.installed_program_ref_path(program_id, program_version);

        match fs::read(path).await {
            Ok(bytes) => {
                let rec: compute_abi::InstalledProgramRecord = serde_json::from_slice(&bytes)?;
                Ok(Some(rec))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn list_installed_programs(&self) -> Result<Vec<compute_abi::InstalledProgramRecord>> {
        let mut out = Vec::new();
        let mut rd = fs::read_dir(&self.program_refs_dir).await?;

        while let Some(ent) = rd.next_entry().await? {
            let path = ent.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).await?;
            let rec: compute_abi::InstalledProgramRecord = serde_json::from_slice(&bytes)?;
            out.push(rec);
        }

        Ok(out)
    }

    pub fn artifact_alias_path(&self, namespace: &str, logical_name: &str) -> PathBuf {
        self.artifact_aliases_dir
            .join(format!("{}__{}.json", sanitize(namespace), sanitize(logical_name)))
    }

    pub async fn set_alias(
        &self,
        alias: &crate::ArtifactAlias,
        manifest_cid: &BlobId,
    ) -> Result<()> {
        let path = self.artifact_alias_path(&alias.namespace, &alias.logical_name);
        let rec = AliasRecord {
            alias: alias.clone(),
            manifest_cid: *manifest_cid,
        };
        let bytes = serde_json::to_vec_pretty(&rec)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    pub async fn get_alias(
        &self,
        alias: &crate::ArtifactAlias,
    ) -> Result<Option<BlobId>> {
        let path = self.artifact_alias_path(&alias.namespace, &alias.logical_name);
        match fs::read(path).await {
            Ok(bytes) => {
                let rec: AliasRecord = serde_json::from_slice(&bytes)?;
                Ok(Some(rec.manifest_cid))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

fn hex32(id: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in id {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}