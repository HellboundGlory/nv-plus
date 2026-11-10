use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use nv_plus_notices::{merged, run, Extra, Options};
use serde_json::{json, Value};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nv-plus-notices-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch folder");
        Scratch(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn package(&self, folder: &str) -> PathBuf {
        let path = self.0.join(folder);
        fs::create_dir_all(&path).expect("package folder");
        path
    }

    fn file(&self, relative: &str, contents: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent folder");
        }
        fs::write(path, contents).expect("write file");
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.0.join(relative)).expect("read file")
    }

    fn out(&self) -> PathBuf {
        self.0.join("out")
    }

    fn options(&self) -> Options {
        Options {
            package: "root".to_owned(),
            out: self.out(),
            extras: Vec::new(),
            check: false,
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package(id: &str, name: &str, folder: &Path, license: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "version": "1.0.0",
        "license": license,
        "repository": "https://example.invalid/repo",
        "authors": ["Someone <someone@example.invalid>"],
        "manifest_path": folder.join("Cargo.toml").to_string_lossy(),
        "source": "registry+https://github.com/rust-lang/crates.io-index",
    })
}

fn root_package(folder: &Path) -> Value {
    json!({
        "id": "root",
        "name": "root",
        "version": "0.1.0",
        "license": null,
        "repository": null,
        "authors": [],
        "manifest_path": folder.join("Cargo.toml").to_string_lossy(),
        "source": null,
    })
}

fn node(deps: &[(&str, Option<&str>)]) -> Value {
    let dependencies: Vec<Value> = deps
        .iter()
        .map(|(pkg, kind)| {
            json!({
                "pkg": pkg,
                "dep_kinds": [{ "kind": kind, "target": null }],
            })
        })
        .collect();
    json!({ "id": "root", "deps": dependencies })
}

fn licence_file(folder: &Path, name: &str, contents: &str) {
    fs::write(folder.join(name), contents).expect("write licence");
}

#[test]
fn only_normal_and_build_dependencies_are_noticed() {
    let scratch = Scratch::new("walk");
    let root = scratch.package("root");
    let normal = scratch.package("normal-1.0.0");
    let build = scratch.package("build-1.0.0");
    let dev = scratch.package("dev-1.0.0");
    let unused = scratch.package("unused-1.0.0");
    for folder in [&normal, &build, &dev, &unused] {
        licence_file(folder, "LICENSE", "text\n");
    }
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("normal", "normal", &normal, "MIT"),
            package("build", "build", &build, "MIT"),
            package("dev", "dev", &dev, "MIT"),
            package("unused", "unused", &unused, "MIT"),
        ],
        "resolve": { "nodes": [
            node(&[("normal", None), ("build", Some("build")), ("dev", Some("dev"))]),
            json!({ "id": "normal", "deps": [] }),
            json!({ "id": "build", "deps": [] }),
            json!({ "id": "dev", "deps": [] }),
        ] },
    });

    run(&metadata.to_string(), &scratch.options()).expect("run");

    let notices = scratch.read("out/LAUNCHER-NOTICES.txt");
    assert!(notices.contains("normal 1.0.0"), "{notices}");
    assert!(notices.contains("build 1.0.0"), "{notices}");
    assert!(!notices.contains("dev 1.0.0"), "{notices}");
    assert!(!notices.contains("unused 1.0.0"), "{notices}");
    assert!(!scratch.out().join("licenses/dev-1.0.0").exists());
    assert!(!scratch.out().join("licenses/unused-1.0.0").exists());
}

#[test]
fn shipped_licence_files_are_copied_and_standard_texts_written_without_one() {
    let scratch = Scratch::new("licences");
    let root = scratch.package("root");
    let two = scratch.package("two-1.0.0");
    let none = scratch.package("none-1.0.0");
    let boost = scratch.package("boost-1.0.0");
    licence_file(&two, "LICENSE-MIT", "shipped mit\n");
    licence_file(&two, "COPYING", "shipped copying\n");
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("two", "two", &two, "MIT"),
            package("none", "none", &none, "MIT OR Apache-2.0"),
            package("boost", "boost", &boost, "BSL-1.0"),
        ],
        "resolve": { "nodes": [
            node(&[("two", None), ("none", None), ("boost", None)]),
            json!({ "id": "two", "deps": [] }),
            json!({ "id": "none", "deps": [] }),
            json!({ "id": "boost", "deps": [] }),
        ] },
    });

    run(&metadata.to_string(), &scratch.options()).expect("run");

    let notices = scratch.read("out/LAUNCHER-NOTICES.txt");
    assert_eq!(
        notices
            .split("\n\ntwo 1.0.0\n")
            .nth(1)
            .and_then(|entry| entry.lines().next()),
        Some("  License expression: MIT"),
        "{notices}"
    );
    assert!(
        notices.contains(
            "  Bundled license files:\n    two-1.0.0/COPYING\n    two-1.0.0/LICENSE-MIT\n"
        ),
        "{notices}"
    );
    assert!(
        notices.contains(
            "  (The package ships no license file: the texts below are written from its license \
         expression; the MIT copyright line names its authors.)\n"
        ),
        "{notices}"
    );
    assert!(
        notices.contains(
            "  Bundled license files:\n    none-1.0.0/LICENSE-APACHE\n    none-1.0.0/LICENSE-MIT\n"
        ),
        "{notices}"
    );
    assert!(
        notices.contains(
            "boost 1.0.0\n  License expression: BSL-1.0\n  Repository: \
             https://example.invalid/repo\n  (The package ships no license file: the texts below \
             are written from its license expression.)\n  Bundled license files:\n    \
             boost-1.0.0/LICENSE-BOOST\n"
        ),
        "{notices}"
    );
    assert_eq!(
        scratch.read("out/licenses/two-1.0.0/LICENSE-MIT"),
        "shipped mit\n"
    );
    assert_eq!(
        scratch.read("out/licenses/two-1.0.0/COPYING"),
        "shipped copying\n"
    );
    assert!(scratch
        .read("out/licenses/none-1.0.0/LICENSE-MIT")
        .contains("MIT License\n\nCopyright (c) Someone\n\nPermission is hereby granted"));
    assert!(scratch
        .read("out/licenses/none-1.0.0/LICENSE-APACHE")
        .starts_with("                                 Apache License\n"));
    assert!(scratch
        .read("out/licenses/boost-1.0.0/LICENSE-BOOST")
        .starts_with(
        "Boost Software License - Version 1.0 - August 17th, 2003\n\nPermission is hereby granted"
    ));
}

#[test]
fn a_package_without_a_file_or_a_standard_expression_is_an_error() {
    let scratch = Scratch::new("zlib");
    let root = scratch.package("root");
    let zlib = scratch.package("zlib-1.0.0");
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("zlib", "zlib", &zlib, "Zlib"),
        ],
        "resolve": { "nodes": [
            node(&[("zlib", None)]),
            json!({ "id": "zlib", "deps": [] }),
        ] },
    });

    let error = run(&metadata.to_string(), &scratch.options()).expect_err("error");

    assert!(error.contains("zlib 1.0.0"), "{error}");
    assert!(error.contains("Zlib"), "{error}");
}

#[test]
fn check_reports_a_changed_file_and_passes_on_a_fresh_write() {
    let scratch = Scratch::new("check");
    let root = scratch.package("root");
    let one = scratch.package("one-1.0.0");
    licence_file(&one, "LICENSE", "shipped\n");
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("one", "one", &one, "MIT"),
        ],
        "resolve": { "nodes": [
            node(&[("one", None)]),
            json!({ "id": "one", "deps": [] }),
        ] },
    });
    let mut options = scratch.options();

    run(&metadata.to_string(), &options).expect("write");
    options.check = true;
    assert_eq!(
        run(&metadata.to_string(), &options).expect("fresh"),
        Vec::<String>::new()
    );

    scratch.file("out/licenses/one-1.0.0/LICENSE", "edited\n");
    let differences = run(&metadata.to_string(), &options).expect("changed");
    assert_eq!(
        differences,
        vec!["changed licenses/one-1.0.0/LICENSE".to_owned()]
    );

    fs::remove_file(scratch.out().join("licenses/one-1.0.0/LICENSE")).expect("remove");
    let differences = run(&metadata.to_string(), &options).expect("missing");
    assert_eq!(
        differences,
        vec!["missing licenses/one-1.0.0/LICENSE".to_owned()]
    );
}

#[test]
fn check_names_a_stale_file_and_a_write_removes_it() {
    let scratch = Scratch::new("check-stale");
    let root = scratch.package("root");
    let one = scratch.package("one-1.0.0");
    licence_file(&one, "LICENSE", "shipped\n");
    scratch.file("out/licenses/gone-0.1.0/LICENSE", "old\n");
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("one", "one", &one, "MIT"),
        ],
        "resolve": { "nodes": [
            node(&[("one", None)]),
            json!({ "id": "one", "deps": [] }),
        ] },
    });
    let mut options = scratch.options();
    options.check = true;
    assert!(run(&metadata.to_string(), &options)
        .expect("check")
        .contains(&"extra licenses/gone-0.1.0/LICENSE".to_owned()));

    options.check = false;
    run(&metadata.to_string(), &options).expect("write");
    assert!(!scratch.out().join("licenses/gone-0.1.0").exists());
    options.check = true;
    assert_eq!(
        run(&metadata.to_string(), &options).expect("check"),
        Vec::<String>::new()
    );
}

#[test]
fn check_ignores_line_ends_git_may_change() {
    let scratch = Scratch::new("line-ends");
    let root = scratch.package("root");
    let one = scratch.package("one-1.0.0");
    let two = scratch.package("two-1.0.0");
    licence_file(&one, "LICENSE", "line one\r\nline two\r\n");
    licence_file(&two, "LICENSE", "line one\nline two\n");
    let metadata = json!({
        "packages": [
            root_package(&root),
            package("one", "one", &one, "MIT"),
            package("two", "two", &two, "MIT"),
        ],
        "resolve": { "nodes": [
            node(&[("one", None), ("two", None)]),
            json!({ "id": "one", "deps": [] }),
            json!({ "id": "two", "deps": [] }),
        ] },
    });
    let mut options = scratch.options();
    run(&metadata.to_string(), &options).expect("write");
    scratch.file("out/licenses/one-1.0.0/LICENSE", "line one\nline two\n");
    scratch.file("out/licenses/two-1.0.0/LICENSE", "line one\r\nline two\r\n");
    let notices = scratch
        .read("out/LAUNCHER-NOTICES.txt")
        .replace('\n', "\r\n");
    scratch.file("out/LAUNCHER-NOTICES.txt", &notices);

    options.check = true;
    assert_eq!(
        run(&metadata.to_string(), &options).expect("check"),
        Vec::<String>::new()
    );
}

#[test]
fn an_mit_package_with_no_named_author_gets_a_holder_all_the_same() {
    let scratch = Scratch::new("no-author");
    let root = scratch.package("root");
    let bare = scratch.package("bare-1.0.0");
    let mut bare_package = package("bare", "bare", &bare, "MIT");
    bare_package["authors"] = json!(["<someone@example.invalid>"]);
    let metadata = json!({
        "packages": [root_package(&root), bare_package],
        "resolve": { "nodes": [node(&[("bare", None)]), json!({ "id": "bare", "deps": [] })] },
    });

    run(&metadata.to_string(), &scratch.options()).expect("run");

    let mit = scratch.read("out/licenses/bare-1.0.0/LICENSE-MIT");
    assert_eq!(mit.lines().nth(2), Some("Copyright (c) the bare authors"));
    assert!(!scratch
        .read("out/LAUNCHER-NOTICES.txt")
        .contains("names its authors"));
}

#[test]
fn extras_are_copied_and_listed() {
    let scratch = Scratch::new("extras");
    let root = scratch.package("root");
    scratch.file("fonts/OFL.txt", "font licence\n");
    let metadata = json!({
        "packages": [root_package(&root)],
        "resolve": { "nodes": [node(&[])] },
    });
    let mut options = scratch.options();
    options.extras = vec![Extra {
        name: "Barlow".to_owned(),
        path: scratch.path().join("fonts/OFL.txt"),
    }];

    run(&metadata.to_string(), &options).expect("run");

    let notices = scratch.read("out/LAUNCHER-NOTICES.txt");
    assert!(
        notices.contains("\nBarlow\n  Bundled license file:\n    Barlow/OFL.txt\n"),
        "{notices}"
    );
    assert_eq!(
        scratch.read("out/licenses/Barlow/OFL.txt"),
        "font licence\n"
    );
}

#[test]
fn every_target_given_is_noticed() {
    let scratch = Scratch::new("targets");
    let root = scratch.package("root");
    let gnu = scratch.package("gnu-1.0.0");
    let msvc = scratch.package("msvc-1.0.0");
    for folder in [&gnu, &msvc] {
        licence_file(folder, "LICENSE", "text\n");
    }
    let for_target = |dep: &str, folder: &Path| {
        json!({
            "packages": [root_package(&root), package(dep, dep, folder, "MIT")],
            "resolve": { "nodes": [node(&[(dep, None)]), json!({ "id": dep, "deps": [] })] },
        })
        .to_string()
    };
    let metadata = merged(&[for_target("gnu", &gnu), for_target("msvc", &msvc)]).expect("merge");

    run(&metadata, &scratch.options()).expect("run");

    let notices = scratch.read("out/LAUNCHER-NOTICES.txt");
    assert!(notices.contains("gnu 1.0.0"), "{notices}");
    assert!(notices.contains("msvc 1.0.0"), "{notices}");
}
