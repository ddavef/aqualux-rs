// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

// Phase 1 custom pack store; UI integration will consume these in Phase 2/3.
#![allow(dead_code)]

use crate::game::MetricResult;
use crate::game::level_io::{CustomPackFile, MAX_CUSTOM_XML_BYTES, parse_pack_xml, write_pack_xml};
use crate::save::LevelRecord;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum PackSource {
    UserDir,
    PortableDir,
    CliDir,
}

#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub id: String,
    pub path: PathBuf,
    pub source: PackSource,
    pub writable: bool,
    pub pack: CustomPackFile,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomProgress {
    pub packs: HashMap<String, HashMap<usize, LevelRecord>>,
}

#[derive(Debug, Clone)]
pub struct CustomPackStore {
    pub primary_dir: PathBuf,
    pub extra_dirs: Vec<(PathBuf, PackSource)>,
    pub packs: Vec<LoadedPack>,
    pub progress: CustomProgress,
    pub progress_path: PathBuf,
}

pub fn validate_pack_dir(dir: &Path) -> Result<(), String> {
    if dir.as_os_str().is_empty()
        || dir.parent().is_none()
        || dir.file_name().is_none()
        || dir.components().any(|c| matches!(c, Component::ParentDir))
        || !matches!(dir.components().next_back(), Some(Component::Normal(_)))
    {
        return Err(format!(
            "Refusing unsafe custom packs directory '{}'",
            dir.display()
        ));
    }
    if let Ok(meta) = fs::symlink_metadata(dir)
        && (meta.file_type().is_symlink() || !meta.file_type().is_dir())
    {
        return Err(format!(
            "Refusing symlink or non-directory custom packs path '{}'",
            dir.display()
        ));
    }
    Ok(())
}

fn reject_if_symlink_or_non_file(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("Refusing symlink path '{}'", path.display()))
        }
        Ok(meta) if !meta.file_type().is_file() => {
            Err(format!("Refusing non-regular file '{}'", path.display()))
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Cannot inspect '{}': {}", path.display(), e)),
    }
}

fn validate_direct_xml_child(dir: &Path, path: &Path) -> Result<(), String> {
    validate_pack_dir(dir)?;
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("Refusing path traversal in '{}'", path.display()));
    }
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return Err(format!("Invalid pack filename '{}'", path.display()));
    };
    if file_name.is_empty() || file_name.contains(['/', '\\', '\0']) {
        return Err(format!("Unsafe pack filename '{}'", file_name));
    }
    let mut comps = Path::new(file_name).components();
    if !matches!(
        (comps.next(), comps.next()),
        (Some(Component::Normal(_)), None)
    ) {
        return Err(format!("Unsafe pack filename component '{}'", file_name));
    }
    let is_xml = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"));
    if !is_xml {
        return Err(format!("Refusing non-XML file '{}'", path.display()));
    }
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Cannot inspect '{}': {}", path.display(), e))?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return Err(format!("Refusing symlink or non-file '{}'", path.display()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("Missing parent directory for '{}'", path.display()))?;
    let canon_dir =
        fs::canonicalize(dir).map_err(|e| format!("Cannot resolve '{}': {}", dir.display(), e))?;
    let canon_parent = fs::canonicalize(parent)
        .map_err(|e| format!("Cannot resolve '{}': {}", parent.display(), e))?;
    if canon_parent != canon_dir {
        return Err(format!(
            "Refusing file outside custom packs directory '{}'",
            path.display()
        ));
    }
    let canon_path = fs::canonicalize(path)
        .map_err(|e| format!("Cannot resolve '{}': {}", path.display(), e))?;
    if canon_path != canon_dir.join(file_name) {
        return Err(format!(
            "Refusing unexpected canonical target '{}'",
            path.display()
        ));
    }
    Ok(())
}

fn validate_safe_file_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.file_name().is_none()
        || path.components().any(|c| matches!(c, Component::ParentDir))
        || !matches!(path.components().next_back(), Some(Component::Normal(_)))
    {
        return Err(format!("Refusing unsafe file path '{}'", path.display()));
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Ok(meta) = fs::symlink_metadata(parent)
        && (meta.file_type().is_symlink() || !meta.file_type().is_dir())
    {
        return Err(format!(
            "Refusing symlink or non-directory parent '{}'",
            parent.display()
        ));
    }
    reject_if_symlink_or_non_file(path)
}

pub fn sanitize_pack_id(name: &str) -> String {
    let mut cleaned = String::with_capacity(48);
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            cleaned.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch == '_' {
            cleaned.push('_');
            last_dash = false;
        } else if !last_dash && !cleaned.is_empty() {
            cleaned.push('-');
            last_dash = true;
        }
    }
    let trimmed = cleaned.trim_matches(|c| c == '-' || c == '_');
    let truncated: String = trimmed.chars().take(48).collect();
    let base = truncated.trim_matches(|c| c == '-' || c == '_');
    if base.is_empty() || base == "." || base == ".." || is_windows_reserved(base) {
        if base.is_empty() || base == "." || base == ".." {
            "pack".to_string()
        } else {
            let mut prefixed = format!("pack-{}", base);
            prefixed.truncate(48);
            prefixed
        }
    } else {
        base.to_string()
    }
}

fn is_windows_reserved(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    )
}

pub fn resolve_dirs(cli_override: Option<&Path>) -> (PathBuf, Vec<(PathBuf, PackSource)>, PathBuf) {
    let data_root = ProjectDirs::from("com", "aqualux-rs", "aqualux-rs")
        .map(|p| p.data_local_dir().to_path_buf());

    let primary_dir = if let Some(cli) = cli_override {
        cli.to_path_buf()
    } else if let Some(env_dir) = std::env::var_os("AQUALUX_CUSTOM_LEVELS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        env_dir
    } else if let Some(ref root) = data_root {
        root.join("custom_packs")
    } else {
        PathBuf::from("custom_packs")
    };

    let progress_path = if let Some(ref root) = data_root {
        root.join("custom_progress.json")
    } else {
        PathBuf::from("custom_progress.json")
    };

    let mut seen = Vec::new();
    seen.push(normalized_dir_key(&primary_dir));

    let mut extra_dirs = Vec::new();
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        candidates.push(exe_dir.join("custom_packs"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("custom_packs"));
    }

    for dir in candidates {
        let key = normalized_dir_key(&dir);
        if !seen.contains(&key) {
            seen.push(key);
            extra_dirs.push((dir, PackSource::PortableDir));
        }
    }

    (primary_dir, extra_dirs, progress_path)
}

fn normalized_dir_key(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(p))
                .unwrap_or_else(|_| p.to_path_buf())
        }
    })
}

impl CustomPackStore {
    pub fn load(cli_override: Option<&Path>) -> Self {
        let (primary_dir, extra_dirs, progress_path) = resolve_dirs(cli_override);
        let mut store = Self::load_with_dirs(primary_dir, extra_dirs, progress_path);
        if cli_override.is_some() {
            for p in &mut store.packs {
                if p.writable {
                    p.source = PackSource::CliDir;
                }
            }
        }
        store
    }

    pub fn load_with_dirs(
        primary_dir: PathBuf,
        extra_dirs: Vec<(PathBuf, PackSource)>,
        progress_path: PathBuf,
    ) -> Self {
        let progress = fs::read_to_string(&progress_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<CustomProgress>(&raw).ok())
            .unwrap_or_default();

        let mut store = Self {
            primary_dir,
            extra_dirs,
            packs: Vec::new(),
            progress,
            progress_path,
        };
        store.reload();
        store
    }

    pub fn reload(&mut self) {
        self.packs.clear();
        let mut seen_ids = HashSet::new();

        self.scan_dir(
            &self.primary_dir.clone(),
            PackSource::UserDir,
            true,
            &mut seen_ids,
        );
        let extras = self.extra_dirs.clone();
        for (dir, src) in extras {
            self.scan_dir(&dir, src, false, &mut seen_ids);
        }

        self.packs.sort_by(|a, b| {
            a.pack
                .meta
                .name
                .to_lowercase()
                .cmp(&b.pack.meta.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
    }

    fn scan_dir(
        &mut self,
        dir: &Path,
        source: PackSource,
        writable: bool,
        seen_ids: &mut HashSet<String>,
    ) {
        if writable && validate_pack_dir(dir).is_err() {
            return;
        }
        if let Ok(meta) = fs::symlink_metadata(dir)
            && (meta.file_type().is_symlink() || !meta.file_type().is_dir())
        {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                fs::symlink_metadata(p)
                    .is_ok_and(|m| !m.file_type().is_symlink() && m.file_type().is_file())
                    && p.extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
            })
            .collect();
        paths.sort();

        for path in paths {
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink()
                || !meta.file_type().is_file()
                || meta.len() as usize > MAX_CUSTOM_XML_BYTES
            {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Custom Pack");
            let id = sanitize_pack_id(stem);
            if !seen_ids.insert(id.clone()) {
                continue;
            }
            let Ok(pack) = parse_pack_xml(&raw, stem) else {
                seen_ids.remove(&id);
                continue;
            };
            self.packs.push(LoadedPack {
                id,
                path,
                source,
                writable,
                pack,
            });
        }
    }

    pub fn save_pack(&mut self, id: &str, pack: CustomPackFile) -> Result<PathBuf, String> {
        let clean_id = sanitize_pack_id(id);
        validate_pack_dir(&self.primary_dir)?;
        fs::create_dir_all(&self.primary_dir)
            .map_err(|e| format!("Failed to create '{}': {}", self.primary_dir.display(), e))?;
        validate_pack_dir(&self.primary_dir)?;
        let dest = self.primary_dir.join(format!("{}.xml", clean_id));
        let canon_dir = fs::canonicalize(&self.primary_dir)
            .map_err(|e| format!("Cannot resolve '{}': {}", self.primary_dir.display(), e))?;
        let dest_parent = dest
            .parent()
            .ok_or_else(|| format!("Missing parent directory for '{}'", dest.display()))?;
        let canon_parent = fs::canonicalize(dest_parent)
            .map_err(|e| format!("Cannot resolve '{}': {}", dest_parent.display(), e))?;
        if canon_parent != canon_dir {
            return Err(format!(
                "Refusing write outside custom packs directory '{}'",
                dest.display()
            ));
        }
        reject_if_symlink_or_non_file(&dest)?;
        let xml = write_pack_xml(&pack);
        fs::write(&dest, xml)
            .map_err(|e| format!("Failed to write '{}': {}", dest.display(), e))?;
        self.reload();
        Ok(dest)
    }

    pub fn import_xml_file(&mut self, src: &Path) -> Result<String, String> {
        let meta = fs::symlink_metadata(src)
            .map_err(|e| format!("Cannot access '{}': {}", src.display(), e))?;
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(format!("Refusing symlink or non-file '{}'", src.display()));
        }
        if meta.len() as usize > MAX_CUSTOM_XML_BYTES {
            return Err(format!(
                "XML file '{}' exceeds {} byte limit",
                src.display(),
                MAX_CUSTOM_XML_BYTES
            ));
        }
        let raw = fs::read_to_string(src)
            .map_err(|e| format!("Failed to read '{}': {}", src.display(), e))?;
        let stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Pack");
        let pack = parse_pack_xml(&raw, stem)?;

        let base_id = sanitize_pack_id(stem);
        let id = self.next_available_id(&base_id);
        self.save_pack(&id, pack)?;
        Ok(id)
    }

    fn next_available_id(&self, base_id: &str) -> String {
        let mut candidate = base_id.to_string();
        let mut suffix = 2usize;
        while fs::symlink_metadata(self.primary_dir.join(format!("{}.xml", candidate))).is_ok() {
            let tail = format!("-{}", suffix);
            let max_prefix = 48usize.saturating_sub(tail.len());
            let prefix: String = base_id.chars().take(max_prefix).collect();
            candidate = format!("{}{}", prefix.trim_end_matches(['-', '_']), tail);
            suffix += 1;
        }
        candidate
    }

    pub fn delete_pack(&mut self, id: &str) -> Result<(), String> {
        validate_pack_dir(&self.primary_dir)?;
        let Some(pos) = self.packs.iter().position(|p| p.id == id) else {
            return Err(format!("Pack '{}' not found", id));
        };
        if !self.packs[pos].writable || self.packs[pos].source == PackSource::PortableDir {
            return Err(format!("Pack '{}' is read-only", id));
        }
        let path = self.packs[pos].path.clone();
        validate_direct_xml_child(&self.primary_dir, &path)?;

        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("Cannot inspect '{}': {}", path.display(), e))?;
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(format!("Refusing symlink or non-file '{}'", path.display()));
        }
        if meta.len() as usize > MAX_CUSTOM_XML_BYTES {
            return Err(format!(
                "Refusing to delete '{}' exceeding {} byte limit",
                path.display(),
                MAX_CUSTOM_XML_BYTES
            ));
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom Pack");
        parse_pack_xml(&raw, stem)
            .map_err(|e| format!("Refusing to delete '{}': {}", path.display(), e))?;

        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete '{}': {}", path.display(), e))?;
        self.progress.packs.remove(id);
        self.save_progress();
        self.reload();
        Ok(())
    }

    pub fn record_win(
        &mut self,
        pack_id: &str,
        level_idx: usize,
        res: MetricResult,
        moves: u32,
        time_ms: u32,
    ) {
        self.progress
            .packs
            .entry(pack_id.to_string())
            .or_default()
            .entry(level_idx)
            .or_default()
            .is_better_or_update(res, moves, time_ms);
        self.save_progress();
    }

    pub fn save_progress(&self) {
        if cfg!(test) {
            return;
        }
        let _ = self.save_progress_to_disk();
    }

    pub fn save_progress_to_disk(&self) -> Result<(), String> {
        validate_safe_file_path(&self.progress_path)?;
        if let Some(parent) = self.progress_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create '{}': {}", parent.display(), e))?;
            let meta = fs::symlink_metadata(parent)
                .map_err(|e| format!("Cannot inspect '{}': {}", parent.display(), e))?;
            if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                return Err(format!(
                    "Refusing symlink or non-directory parent '{}'",
                    parent.display()
                ));
            }
        }
        reject_if_symlink_or_non_file(&self.progress_path)?;
        let json = serde_json::to_string_pretty(&self.progress)
            .map_err(|e| format!("Failed to serialize custom progress: {}", e))?;
        fs::write(&self.progress_path, json)
            .map_err(|e| format!("Failed to write '{}': {}", self.progress_path.display(), e))
    }

    pub fn open_packs_folder(&self) -> Result<(), String> {
        validate_pack_dir(&self.primary_dir)?;
        fs::create_dir_all(&self.primary_dir)
            .map_err(|e| format!("Failed to create '{}': {}", self.primary_dir.display(), e))?;
        validate_pack_dir(&self.primary_dir)?;
        let program = if cfg!(target_os = "windows") {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        Command::new(program)
            .arg(&self.primary_dir)
            .spawn()
            .map_err(|e| format!("Failed to launch '{}': {}", program, e))?;
        Ok(())
    }
}
