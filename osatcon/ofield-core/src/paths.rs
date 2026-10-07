use std::path::{Path, PathBuf};

pub fn project_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn assets_dir() -> PathBuf {
    project_root().join("assets")
}

pub fn data_dir() -> PathBuf {
    project_root().join("data")
    
}

pub fn corridors_dir() -> PathBuf {
    data_dir().join("corridors")
}

pub fn resolve_graph_path(p: &str) -> PathBuf {
    let pp = Path::new(p);

    if pp.is_absolute() {
        return corridors_dir().join(
            pp.file_name()
                .unwrap_or_default()
        );
    }

    let c1 = corridors_dir().join(pp);
    if c1.exists() {
        return c1;
    }

    data_dir().join(pp)
}