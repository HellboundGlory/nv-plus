use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub struct Options {
    pub package: String,
    pub out: PathBuf,
    pub extras: Vec<Extra>,
    pub check: bool,
}

pub struct Extra {
    pub name: String,
    pub path: PathBuf,
}

// Upstream's own standard texts (distribution/licenses: block2 0.6.2 for MIT, dispatch2 0.3.1 for
// Apache-2.0) and ryu 1.0.20's Boost text, copied so an upstream crate bump can't break this build.
const MIT_TEMPLATE: &str = include_str!("../templates/LICENSE-MIT");
const APACHE_TEMPLATE: &str = include_str!("../templates/LICENSE-APACHE");
const BOOST_TEMPLATE: &str = include_str!("../templates/LICENSE-BOOST");

const LICENSE_PREFIXES: [&str; 6] = [
    "LICENSE",
    "LICENCE",
    "COPYING",
    "COPYRIGHT",
    "NOTICE",
    "UNLICENSE",
];

struct Package {
    name: String,
    version: String,
    license: String,
    repository: Option<String>,
    authors: Vec<String>,
    folder: PathBuf,
    external: bool,
}

#[derive(Clone, Copy)]
enum Note {
    NamedAuthors,
    Plain,
}

impl Note {
    fn detail(self) -> &'static str {
        match self {
            Note::NamedAuthors => {
                "the texts below are written from its license expression; the MIT copyright line \
                 names its authors"
            }
            Note::Plain => "the texts below are written from its license expression",
        }
    }
}

struct Entry {
    name: String,
    version: Option<String>,
    expression: Option<String>,
    repository: Option<String>,
    note: Option<Note>,
    files: Vec<String>,
}

type Written = Vec<(String, Vec<u8>)>;

/// One metadata for several targets: every package, and each node's dependencies from all of them,
/// so a crate built for only one target (`windows_x86_64_gnu` or `_msvc`) is still noticed.
pub fn merged(metadata_jsons: &[String]) -> Result<String, String> {
    let mut packages: BTreeMap<String, Value> = BTreeMap::new();
    let mut nodes: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for metadata in metadata_jsons {
        let json: Value = serde_json::from_str(metadata).map_err(|e| format!("metadata: {e}"))?;
        for package in json
            .get("packages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(id) = package.get("id").and_then(Value::as_str) {
                packages
                    .entry(id.to_owned())
                    .or_insert_with(|| package.clone());
            }
        }
        let resolved = json
            .get("resolve")
            .and_then(|resolve| resolve.get("nodes"))
            .and_then(Value::as_array)
            .ok_or("metadata: no resolve")?;
        for node in resolved {
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .ok_or("metadata: node without id")?;
            let deps = nodes.entry(id.to_owned()).or_default();
            for dep in node
                .get("deps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if !deps.contains(dep) {
                    deps.push(dep.clone());
                }
            }
        }
    }
    let nodes: Vec<Value> = nodes
        .into_iter()
        .map(|(id, deps)| serde_json::json!({ "id": id, "deps": deps }))
        .collect();
    let packages: Vec<Value> = packages.into_values().collect();
    Ok(serde_json::json!({ "packages": packages, "resolve": { "nodes": nodes } }).to_string())
}

pub fn run(metadata_json: &str, options: &Options) -> Result<Vec<String>, String> {
    let (entries, mut files) = plan(metadata_json, options)?;
    files.push((
        "LAUNCHER-NOTICES.txt".to_owned(),
        notices_text(&entries).into_bytes(),
    ));
    if options.check {
        return differences(&options.out, &files);
    }
    // A crate the lock no longer has must not keep its folder.
    let licenses = options.out.join("licenses");
    match fs::remove_dir_all(&licenses) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("{}: {e}", licenses.display()))
        }
        _ => {}
    }
    for (relative, bytes) in &files {
        let path = options.out.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(Vec::new())
}

fn plan(metadata_json: &str, options: &Options) -> Result<(Vec<Entry>, Written), String> {
    let json: Value = serde_json::from_str(metadata_json).map_err(|e| format!("metadata: {e}"))?;
    let packages = read_packages(&json)?;
    let mut reached = Vec::new();
    for id in reachable(&json, &packages, &options.package)? {
        if let Some(package) = packages.get(&id) {
            if package.external {
                reached.push(package);
            }
        }
    }
    reached.sort_by(|left, right| (&left.name, &left.version).cmp(&(&right.name, &right.version)));
    let mut entries = Vec::new();
    let mut files = Vec::new();
    for package in reached {
        let folder = format!("{}-{}", package.name, package.version);
        let mut shipped = licence_files(&package.folder)?;
        let note = if shipped.is_empty() {
            shipped = standard_texts(package, &folder, &mut files)?;
            shipped.sort();
            Some(
                match names(&package.license, "MIT") && copyright_line(&package.authors).is_some() {
                    true => Note::NamedAuthors,
                    false => Note::Plain,
                },
            )
        } else {
            for name in &shipped {
                let source = package.folder.join(name);
                let bytes = fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
                files.push((format!("licenses/{folder}/{name}"), bytes));
            }
            None
        };
        entries.push(Entry {
            name: package.name.clone(),
            version: Some(package.version.clone()),
            expression: Some(package.license.clone()),
            repository: package.repository.clone(),
            note,
            files: shipped
                .iter()
                .map(|name| format!("{folder}/{name}"))
                .collect(),
        });
    }
    for extra in &options.extras {
        let bytes = fs::read(&extra.path).map_err(|e| format!("{}: {e}", extra.path.display()))?;
        let file = extra
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{}: no file name", extra.path.display()))?;
        let shown = format!("{}/{file}", extra.name);
        files.push((format!("licenses/{shown}"), bytes));
        entries.push(Entry {
            name: extra.name.clone(),
            version: None,
            expression: None,
            repository: None,
            note: None,
            files: vec![shown],
        });
    }
    Ok((entries, files))
}

fn read_packages(json: &Value) -> Result<BTreeMap<String, Package>, String> {
    let list = json
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("metadata: no packages")?;
    let mut packages = BTreeMap::new();
    for entry in list {
        let id = text(entry, "id").ok_or("metadata: package without id")?;
        let name = text(entry, "name").ok_or("metadata: package without name")?;
        let manifest_path = text(entry, "manifest_path").unwrap_or_default();
        let folder = Path::new(&manifest_path)
            .parent()
            .unwrap_or(Path::new("."))
            .to_owned();
        let package = Package {
            name,
            version: text(entry, "version").unwrap_or_default(),
            license: text(entry, "license").unwrap_or_default(),
            repository: text(entry, "repository"),
            authors: strings(entry, "authors"),
            folder,
            external: text(entry, "source").is_some(),
        };
        packages.insert(id, package);
    }
    Ok(packages)
}

fn reachable(
    json: &Value,
    packages: &BTreeMap<String, Package>,
    root: &str,
) -> Result<BTreeSet<String>, String> {
    let nodes = json
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(Value::as_array)
        .ok_or("metadata: no resolve")?;
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in nodes {
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .ok_or("metadata: node without id")?;
        let mut kept = Vec::new();
        for dep in node
            .get("deps")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if !built(dep) {
                continue;
            }
            if let Some(pkg) = dep.get("pkg").and_then(Value::as_str) {
                kept.push(pkg);
            }
        }
        edges.insert(id, kept);
    }
    let root_id = packages
        .iter()
        .find(|(_, package)| package.name == root)
        .map(|(id, _)| id.clone())
        .ok_or_else(|| format!("metadata: no package named {root}"))?;
    let mut seen = BTreeSet::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(deps) = edges.get(id.as_str()) {
            stack.extend(deps.iter().map(|dep| (*dep).to_owned()));
        }
    }
    Ok(seen)
}

fn built(dep: &Value) -> bool {
    dep.get("dep_kinds")
        .and_then(Value::as_array)
        .is_some_and(|kinds| {
            kinds.iter().any(|kind| match kind.get("kind") {
                None | Some(Value::Null) => true,
                Some(Value::String(name)) => name == "build",
                Some(_) => false,
            })
        })
}

fn licence_files(folder: &Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) => return Err(format!("{}: {e}", folder.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !LICENSE_PREFIXES
            .iter()
            .any(|prefix| name.to_uppercase().starts_with(prefix))
        {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", folder.display()))?;
        if kind.is_dir() {
            for inner in
                fs::read_dir(entry.path()).map_err(|e| format!("{}: {e}", folder.display()))?
            {
                let inner = inner.map_err(|e| format!("{}: {e}", folder.display()))?;
                if inner.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    names.push(format!("{name}/{}", inner.file_name().to_string_lossy()));
                }
            }
        } else if kind.is_file() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn standard_texts(
    package: &Package,
    folder: &str,
    files: &mut Written,
) -> Result<Vec<String>, String> {
    let mit = names(&package.license, "MIT");
    let apache = names(&package.license, "Apache-2.0");
    let boost = names(&package.license, "BSL-1.0");
    if !mit && !apache && !boost {
        return Err(format!(
            "{} {}: ships no license file, and its license expression {} names none of MIT, Apache-2.0 or BSL-1.0",
            package.name, package.version, package.license
        ));
    }
    let mut written = Vec::new();
    if mit {
        written.push("LICENSE-MIT".to_owned());
        files.push((
            format!("licenses/{folder}/LICENSE-MIT"),
            mit_text(&package.name, &package.authors).into_bytes(),
        ));
    }
    if apache {
        written.push("LICENSE-APACHE".to_owned());
        files.push((
            format!("licenses/{folder}/LICENSE-APACHE"),
            lf(APACHE_TEMPLATE.as_bytes()),
        ));
    }
    if boost {
        written.push("LICENSE-BOOST".to_owned());
        files.push((
            format!("licenses/{folder}/LICENSE-BOOST"),
            lf(BOOST_TEMPLATE.as_bytes()),
        ));
    }
    Ok(written)
}

fn names(expression: &str, token: &str) -> bool {
    expression
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .any(|word| word.eq_ignore_ascii_case(token))
}

fn copyright_line(authors: &[String]) -> Option<String> {
    let names: Vec<String> = authors
        .iter()
        .map(|author| match (author.find('<'), author.rfind('>')) {
            (Some(start), Some(end)) if end > start => {
                format!("{}{}", &author[..start], &author[end + 1..])
            }
            _ => author.clone(),
        })
        .map(|author| author.trim().to_owned())
        .filter(|author| !author.is_empty())
        .collect();
    match names.is_empty() {
        true => None,
        false => Some(format!("Copyright (c) {}", names.join(", "))),
    }
}

fn mit_text(name: &str, authors: &[String]) -> String {
    let mut text = String::from("MIT License\n\n");
    text.push_str(
        &copyright_line(authors).unwrap_or_else(|| format!("Copyright (c) the {name} authors")),
    );
    text.push('\n');
    for line in MIT_TEMPLATE.lines().skip(3) {
        text.push_str(line);
        text.push('\n');
    }
    text
}

fn notices_text(entries: &[Entry]) -> String {
    let packages = entries
        .iter()
        .filter(|entry| entry.version.is_some())
        .count();
    let mut text = String::new();
    text.push_str("THIRD-PARTY NOTICES — NV+ launcher (Launcher.exe)\n\n");
    text.push_str(&format!(
        "This file indexes the license texts of the {packages} external packages in nv-plus/launcher/Cargo.lock, \
         which Launcher.exe is built from for Windows (the build-time crates included). Package names, \
         versions and license expressions come from each package's Cargo.toml in the local Cargo registry; \
         the files were copied from that exact version there. A package that ships no license file gets the \
         standard text of its license expression (MIT with its authors as the copyright line, Apache-2.0 \
         and BSL-1.0 verbatim), marked in its entry."
    ));
    text.push_str("\n\n");
    text.push_str(
        "Files are bundled under licenses/<package>-<version>/ beside upstream's license files \
         (nv-plus/distribution/licenses/ in the source tree). No crate source or game assets are included.",
    );
    text.push('\n');
    for entry in entries {
        text.push('\n');
        match &entry.version {
            Some(version) => {
                text.push_str(&format!("{} {version}\n", entry.name));
                text.push_str(&format!(
                    "  License expression: {}\n",
                    entry.expression.as_deref().unwrap_or_default()
                ));
                if let Some(repository) = &entry.repository {
                    text.push_str(&format!("  Repository: {repository}\n"));
                }
                if let Some(note) = entry.note {
                    text.push_str(&format!(
                        "  (The package ships no license file: {}.)\n",
                        note.detail()
                    ));
                }
                text.push_str("  Bundled license files:\n");
            }
            None => {
                text.push_str(&format!("{}\n", entry.name));
                text.push_str("  Bundled license file:\n");
            }
        }
        for file in &entry.files {
            text.push_str(&format!("    {file}\n"));
        }
    }
    text
}

fn differences(out: &Path, files: &[(String, Vec<u8>)]) -> Result<Vec<String>, String> {
    let mut reported = Vec::new();
    let mut written = BTreeSet::new();
    for (relative, bytes) in files {
        written.insert(relative.as_str());
        let path = out.join(relative);
        match fs::read(&path) {
            // Git may turn line ends either way on checkout.
            Ok(current) if lf(&current) == lf(bytes) => {}
            Ok(_) => reported.push(format!("changed {relative}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                reported.push(format!("missing {relative}"))
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    let mut present = Vec::new();
    collect(&out.join("licenses"), "licenses", &mut present)?;
    for relative in present {
        if !written.contains(relative.as_str()) {
            reported.push(format!("extra {relative}"));
        }
    }
    reported.sort();
    Ok(reported)
}

fn collect(directory: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{}: {e}", directory.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", directory.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}/{name}");
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", directory.display()))?;
        if kind.is_dir() {
            collect(&entry.path(), &relative, out)?;
        } else {
            out.push(relative);
        }
    }
    Ok(())
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, &byte) in bytes.iter().enumerate() {
        if !(byte == b'\r' && bytes.get(i + 1) == Some(&b'\n')) {
            out.push(byte);
        }
    }
    out
}
