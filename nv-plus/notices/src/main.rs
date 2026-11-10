use std::path::PathBuf;
use std::process::{Command, ExitCode};

use nv_plus_notices::{Extra, Options};

const USAGE: &str =
    "usage: nv-plus-notices --manifest <Cargo.toml> --package <name> --target <triple>... \
                     --out <dir> [--extra <name>=<file>]... [--check]";

struct Arguments {
    manifest: PathBuf,
    targets: Vec<String>,
    options: Options,
}

fn main() -> ExitCode {
    let arguments = match parse(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match generate(&arguments) {
        Ok(differences) if differences.is_empty() => ExitCode::SUCCESS,
        Ok(differences) => {
            for line in differences {
                println!("{line}");
            }
            ExitCode::FAILURE
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn generate(arguments: &Arguments) -> Result<Vec<String>, String> {
    let jsons = arguments
        .targets
        .iter()
        .map(|target| metadata(&arguments.manifest, target))
        .collect::<Result<Vec<_>, _>>()?;
    nv_plus_notices::run(&nv_plus_notices::merged(&jsons)?, &arguments.options)
}

fn metadata(manifest: &PathBuf, target: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout).map_err(|e| format!("cargo metadata: {e}"))
}

fn parse(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut manifest = None;
    let mut package = None;
    let mut targets = Vec::new();
    let mut out = None;
    let mut extras = Vec::new();
    let mut check = false;
    let mut arguments = arguments;
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value("--manifest")?)),
            "--package" => package = Some(value("--package")?),
            "--target" => targets.push(value("--target")?),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--extra" => extras.push(parse_extra(&value("--extra")?)?),
            "--check" => check = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Arguments {
        manifest: manifest.ok_or("--manifest is required")?,
        targets: match targets.is_empty() {
            true => return Err("--target is required".to_owned()),
            false => targets,
        },
        options: Options {
            package: package.ok_or("--package is required")?,
            out: out.ok_or("--out is required")?,
            extras,
            check,
        },
    })
}

fn parse_extra(value: &str) -> Result<Extra, String> {
    let (name, path) = value
        .split_once('=')
        .ok_or_else(|| format!("--extra needs <name>=<file>, got {value}"))?;
    Ok(Extra {
        name: name.to_owned(),
        path: PathBuf::from(path),
    })
}
