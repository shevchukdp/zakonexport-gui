//! Output naming policy: template, sanitization, dedupe, per-run folder.
//! Optionally loaded from <baza>\naming.toml.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Naming {
    pub out_dir: String,
    pub template: String,
    pub per_run_folder: bool,
}

impl Default for Naming {
    fn default() -> Self {
        Self {
            out_dir: "C:\\temp\\opencode\\export".to_string(),
            template: "{index:05}_{chunk}.{format}".to_string(),
            per_run_folder: true,
        }
    }
}

impl Naming {
    /// Load from <baza>\naming.toml (flat or [naming]-section layout). Missing = default.
    pub fn load(baza: &Path) -> Self {
        let p = baza.join("naming.toml");
        if let Ok(raw) = std::fs::read_to_string(&p) {
            if let Ok(n) = toml::from_str::<Naming>(&raw) {
                return n;
            }
            #[derive(Deserialize)]
            struct Wrap { naming: Naming }
            if let Ok(w) = toml::from_str::<Wrap>(&raw) {
                return w.naming;
            }
        }
        Naming::default()
    }

    /// Render the template with {index}/{index:05N}/{chunk}/{format} tokens.
    pub fn render(&self, index: usize, chunk: &str, format: &str) -> String {
        let mut t = self.template.clone();
        // {index:05} style padding
        if let Some(pos) = t.find("{index:") {
            if let Some(end) = t[pos..].find('}') {
                let spec = &t[pos + 7..pos + end];
                let digits = spec.chars().filter(|c| c.is_ascii_digit()).count();
                if digits > 0 {
                    let sub = format!("{:0width$}", index, width = digits);
                    t.replace_range(pos..pos + end + 1, &sub);
                }
            }
        }
        t = t.replace("{index}", &index.to_string());
        t = t.replace("{chunk}", chunk.trim_end_matches(".mb"));
        t = t.replace("{format}", format);
        t
    }

    /// Sanitize a filename: replace Windows-hostile chars.
    pub fn sanitize(&self, name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if (c as u32) < 0x20 => '_',
                c => c,
            })
            .collect()
    }

    /// Create the output directory, adding run_<unix_ts> if per_run_folder.
    pub fn prepare_out(&self, out: &Path) -> Result<PathBuf> {
        let dir = if self.per_run_folder {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.join(format!("run_{ts}"))
        } else {
            out.to_path_buf()
        };
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create output dir {}", dir.display()))?;
        Ok(dir)
    }

    /// Append _1.._999 if the path already exists.
    pub fn dedupe(&self, path: &Path) -> PathBuf {
        if !path.exists() {
            return path.to_path_buf();
        }
        let stem = path.with_extension("").to_string_lossy().to_string();
        let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        for i in 1..1000 {
            let c = PathBuf::from(format!("{stem}_{i}{ext}"));
            if !c.exists() {
                return c;
            }
        }
        path.to_path_buf()
    }
}
