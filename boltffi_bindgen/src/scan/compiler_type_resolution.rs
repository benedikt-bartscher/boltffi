use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use proc_macro2::Span;
use quote::quote;
use serde::Deserialize;
use syn::{LitStr, Type};

fn runner_dir(crate_path: &Path) -> PathBuf {
    crate_path
        .join("target")
        .join("boltffi_bindgen_type_resolution")
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    let existing = fs::read_to_string(path).ok();
    if existing.as_deref() == Some(contents) {
        return Ok(());
    }
    fs::write(path, contents).map_err(|e| format!("write {}: {}", path.display(), e))
}

fn rewrite_crate_prefix(spelling: &str) -> Option<String> {
    spelling
        .strip_prefix("crate::")
        .map(|rest| format!("target_crate::{}", rest))
}

fn generate_main_rs(spellings: &[(String, Type)]) -> String {
    let entries = spellings.iter().map(|(original, ty)| {
        let lit = LitStr::new(original, Span::call_site());
        quote! {
            {
                let canonical = ::std::any::type_name::<#ty>();
                let _ = writeln!(out, "{}\t{}", #lit, canonical);
            }
        }
    });

    quote! {
        use ::std::io::{self, Write};

        use target_crate as _;

        fn main() {
            let mut out = io::BufWriter::new(io::stdout());
            #(#entries)*
        }
    }
    .to_string()
}

#[derive(Clone, Debug, Deserialize)]
struct CargoMetadataJson {
    packages: Vec<CargoPackage>,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoPackage {
    name: String,
    edition: String,
    manifest_path: String,
}

fn load_cargo_metadata(crate_path: &Path) -> Result<CargoMetadataJson, String> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .current_dir(crate_path)
        .output()
        .map_err(|e| format!("cargo metadata: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo metadata failed: {}", stderr.trim()));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("parse cargo metadata: {}", e))
}

fn select_target_package(
    crate_path: &Path,
    package_hint: &str,
    metadata: &CargoMetadataJson,
) -> Result<CargoPackage, String> {
    let canonical_manifest_path = crate_path
        .join("Cargo.toml")
        .canonicalize()
        .ok()
        .and_then(|path| path.to_str().map(str::to_string));

    metadata
        .packages
        .iter()
        .find(|package| Some(package.manifest_path.as_str()) == canonical_manifest_path.as_deref())
        .cloned()
        .or_else(|| {
            metadata
                .packages
                .iter()
                .find(|package| package.name == package_hint)
                .cloned()
        })
        .ok_or_else(|| {
            let available = metadata
                .packages
                .iter()
                .map(|package| format!("{} ({})", package.name, package.manifest_path))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "could not select target package (hint: {}) from cargo metadata: {}",
                package_hint, available
            )
        })
}

fn cargo_manifest_dir(manifest_path: &str) -> Result<PathBuf, String> {
    PathBuf::from(manifest_path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("invalid manifest path: {}", manifest_path))
}

fn runner_lock(crate_path: &Path) -> Result<Arc<Mutex<()>>, String> {
    static RUNNER_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

    let key = crate_path
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {}", crate_path.display(), e))?;
    let locks = RUNNER_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .map_err(|_| "runner lock registry poisoned".to_string())?;

    Ok(locks
        .entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

pub fn resolve(
    crate_path: &Path,
    package_hint: &str,
    spellings: impl IntoIterator<Item = String>,
) -> Result<HashMap<String, String>, String> {
    let mut unique = HashSet::<String>::new();
    let mut targets = spellings
        .into_iter()
        .filter(|s| s.starts_with("crate::"))
        .filter(|s| unique.insert(s.clone()))
        .collect::<Vec<_>>();

    targets.sort();

    if targets.is_empty() {
        return Ok(HashMap::new());
    }

    let parsed = targets
        .iter()
        .filter_map(|original| {
            let rewritten = rewrite_crate_prefix(original)?;
            let ty = syn::parse_str::<Type>(&rewritten).ok()?;
            Some((original.clone(), ty))
        })
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        return Ok(HashMap::new());
    }

    let metadata = load_cargo_metadata(crate_path)?;
    let target_package = select_target_package(crate_path, package_hint, &metadata)?;
    let target_manifest_dir = cargo_manifest_dir(&target_package.manifest_path)?;
    let lock = runner_lock(crate_path)?;
    let _guard = lock
        .lock()
        .map_err(|_| "type resolution runner lock poisoned".to_string())?;

    let dir = runner_dir(crate_path);
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| format!("mkdir {}: {}", src_dir.display(), e))?;

    let cargo_toml = format!(
        "[workspace]\n\n[package]\nname = \"boltffi_bindgen_type_resolution_runner\"\nversion = \"0.1.0\"\nedition = \"{}\"\n\n[dependencies]\ntarget_crate = {{ path = '{}', package = \"{}\" }}\n",
        target_package.edition,
        target_manifest_dir.display(),
        target_package.name,
    );

    write_if_changed(&dir.join("Cargo.toml"), &cargo_toml)?;
    write_if_changed(&src_dir.join("main.rs"), &generate_main_rs(&parsed))?;

    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("run type resolution runner: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(resolution_failure_message(&stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let map = stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(spelling, canonical)| (spelling.to_string(), canonical.to_string()))
        .collect::<HashMap<_, _>>();

    Ok(map)
}

/// Formats a runner build failure into the error the user sees.
///
/// The runner names every exported type by its source-location path
/// (`crate::inner::Widget`) from outside the target crate, so a
/// `#[data]`/`#[export]` item inside a private module fails the build
/// with rustc's E0603 even when the item itself is re-exported from a
/// public module (`mod inner; pub use inner::*;`). Translate that case
/// into an actionable message instead of raw compiler output.
fn resolution_failure_message(stderr: &str) -> String {
    let stderr = stderr.trim();
    if !stderr.contains("error[E0603]") {
        return format!("type resolution runner failed: {}", stderr);
    }
    let private_paths = stderr
        .lines()
        .filter_map(|line| line.trim().strip_prefix("error[E0603]: "))
        .collect::<Vec<_>>()
        .join("; ");
    let detail = if private_paths.is_empty() {
        String::new()
    } else {
        format!(" ({})", private_paths)
    };
    format!(
        "exported #[data]/#[export] types must be reachable through a public module path{}: \
         boltffi_bindgen references each type by the module path where it is declared, so a \
         `pub use` re-export from a private module is not enough. Declare the containing \
         module `pub` (e.g. `pub mod inner;`) or move the type into a public module.\n\
         full compiler output:\n{}",
        detail, stderr
    )
}

#[cfg(test)]
mod tests {
    use super::resolution_failure_message;

    #[test]
    fn e0603_failure_maps_to_actionable_visibility_message() {
        let stderr = "error[E0603]: module `inner` is private\n \
                      --> src/main.rs:5:44\n  |\n5 | ...crate::inner::Widget...\n";

        let message = resolution_failure_message(stderr);

        assert!(
            message.contains("reachable through a public module path"),
            "expected actionable visibility guidance, got: {message}"
        );
        assert!(
            message.contains("module `inner` is private"),
            "expected the private module to be named, got: {message}"
        );
        assert!(
            message.contains("pub mod inner;"),
            "expected a concrete fix suggestion, got: {message}"
        );
        assert!(
            message.contains("error[E0603]"),
            "expected the raw compiler output to be preserved, got: {message}"
        );
    }

    #[test]
    fn non_e0603_failure_keeps_raw_runner_error() {
        let stderr = "error[E0432]: unresolved import `target_crate`\n";

        let message = resolution_failure_message(stderr);

        assert!(
            message.starts_with("type resolution runner failed: "),
            "expected the generic runner error, got: {message}"
        );
        assert!(!message.contains("public module path"));
    }
}
