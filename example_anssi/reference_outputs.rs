//! Génère les trois sorties de référence demandées au point 3 c) de la liste des éléments
//! techniques du formulaire ANSSI (décret n° 2007-663).
//!
//!     cargo run --example reference_outputs
//!
//! Les textes clairs sont les fichiers de `example_anssi/reference_sources/`, livrés tels
//! quels. Les cryptogrammes et un `MANIFEST.txt` récapitulant clé, nonces, tailles et
//! identifiants de contenu sont écrits dans `example_anssi/reference_outputs/`.
//!
//! La clé et les nonces sont **fixes et arbitraires**, jamais tirés au hasard. Le
//! formulaire demande « une clé choisie arbitrairement », et l'objet du point 3 c) est de
//! permettre à l'Agence de vérifier la mise en œuvre : elle doit donc pouvoir rejouer ce
//! programme et retrouver les mêmes octets. XChaCha20-Poly1305 étant déterministe à clé et
//! nonce donnés, les identifiants de contenu sont stables d'une exécution à l'autre.
//!
//! En fonctionnement, le moyen n'emploie jamais de clé fixe : `content_hashing::newkey()`
//! tire clé et nonce du générateur d'aléa du système, à nouveau pour chaque fichier.

use std::{fs, path::Path};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Nombre de blocs de 2 Mio que couvre un clair de cette taille.
fn blocks(len: u64) -> u64 {
    const BLOCK: u64 = 2 * 1024 * 1024;
    if len == 0 { 1 } else { len.div_ceil(BLOCK) }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sources = Path::new("example_anssi/reference_sources");
    let dir = Path::new("example_anssi/reference_outputs");
    fs::create_dir_all(dir)?;

    // `content_hashing::encrypt_file` écrit le cryptogramme dans `get_storage_dir()`, qui
    // vaut `./data/content/` par défaut. On le redirige vers le répertoire de l'example_ANSSI
    // pour que clair et chiffré soient livrés ensemble. La barre finale est nécessaire :
    // le chemin est construit par concaténation directe avec l'identifiant de contenu.
    std::env::set_var("STORAGE_DIR", format!("{}/", dir.display()));

    // Clé fixe : 00 01 02 … 1f
    let key: [u8; 32] = std::array::from_fn(|i| i as u8);

    // Trois cas : un bloc unique, une dizaine de blocs, et le cas limite du fichier vide.
    let cases: [(&str, &str); 3] = [
        ("CyberDico_ANSSI.pdf", "document PDF, un seul bloc"),
        ("odetector_debris_vrona.gif", "image GIF animée, plusieurs blocs"),
        ("clair_vide.bin", "fichier vide, cas limite"),
    ];

    // Le clair vide est engendré ici plutôt que versionné, un fichier de zéro octet se
    // perdant trop facilement au fil des copies et des archives.
    let empty = sources.join("clair_vide.bin");
    if !empty.exists() {
        fs::create_dir_all(sources)?;
        fs::write(&empty, b"")?;
    }

    let mut manifest = String::new();
    manifest.push_str("ØMYSTIK - sorties de reference\n");
    manifest.push_str("Point 3 c) des elements techniques, decret n 2007-663\n\n");
    manifest.push_str("Algorithme   : XChaCha20-Poly1305, chiffrement authentifie par flux\n");
    manifest.push_str("Taille bloc  : 2 Mio\n");
    manifest.push_str("Etiquette    : Poly1305, 16 octets par bloc\n");
    manifest.push_str("Identifiant  : empreinte BLAKE3 du cryptogramme, extension .bin\n");
    manifest.push_str("Clairs       : example_anssi/reference_sources/\n");
    manifest.push_str(&format!("Cle (32 o)   : {}\n\n", hex(&key)));

    println!("cle : {}", hex(&key));

    for (i, (name, description)) in cases.into_iter().enumerate() {
        // Nonce fixe a0 a1 … b2, dernier octet propre à chaque cas.
        let mut nonce: [u8; 19] = std::array::from_fn(|j| 0xa0 + j as u8);
        nonce[18] = i as u8;

        let path = sources.join(name);
        let clear_len = fs::metadata(&path)
            .map_err(|e| format!("clair introuvable : {} ({e})", path.display()))?
            .len();

        let cid = content_hashing::encrypt_file(&key, &nonce, path.to_str().unwrap()).await?;

        let cipher_len = fs::metadata(dir.join(format!("{cid}.bin")))?.len();
        let n_blocks = blocks(clear_len);

        println!(
            "{name} ({clear_len} octets, {n_blocks} bloc(s)) | nonce {} | CID {cid}",
            hex(&nonce)
        );

        manifest.push_str(&format!("Cas {}\n", i + 1));
        manifest.push_str(&format!("  description     : {description}\n"));
        manifest.push_str(&format!("  clair           : {name}\n"));
        manifest.push_str(&format!("  taille claire   : {clear_len} octets\n"));
        manifest.push_str(&format!("  blocs           : {n_blocks}\n"));
        manifest.push_str(&format!("  nonce (19 o)    : {}\n", hex(&nonce)));
        manifest.push_str(&format!("  cryptogramme    : {cid}.bin\n"));
        manifest.push_str(&format!("  taille chiffree : {cipher_len} octets\n"));
        manifest.push_str(&format!(
            "  surcout         : {} octets, soit {} x 16\n\n",
            cipher_len - clear_len,
            (cipher_len - clear_len) / 16
        ));
    }

    let manifest_path = dir.join("MANIFEST.txt");
    fs::write(&manifest_path, manifest)?;
    println!("\nmanifeste : {}", manifest_path.display());

    Ok(())
}
