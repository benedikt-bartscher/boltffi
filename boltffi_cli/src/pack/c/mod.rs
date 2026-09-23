//! C package assembler.
//!
//! The C package is intentionally the simplest in the repository: the
//! ergonomic header, a shared library, and a static archive. It is build-system
//! agnostic so downstream users can easily integrate those artifacts into their
//! own build system.
//!
//! ```text
//! dist/c/
//! ├── include/<library>.h
//! └── lib/
//!     ├── lib<library>.{so,dylib,dll}
//!     └── lib<library>.a
//! ```
//!
//! A consumer links with `-I dist/c/include -L dist/c/lib -l<library>` and
//! chooses dynamic or static linking by picking `lib<library>.so` or
//! `lib<library>.a`.

use std::path::PathBuf;

use boltffi_bindgen::target::Target;

use crate::{
    build::{
        BindingExpansion, BuildOptions, BuildSelection, Builder, CargoBuildProfile, OutputCallback,
        resolve_build_profile,
    },
    cargo::Cargo,
    cli::{CliError, Result},
    commands::{
        generate::{GenerateOptions, GenerateTarget, run_generate_with_output},
        pack::PackCOptions,
    },
    config::Config,
    pack::{PackError, print_cargo_line, resolve_build_cargo_args},
    reporter::Reporter,
    target::NativeHostPlatform,
};

fn ensure_c_pack_cargo_args_supported(cargo: &Cargo) -> Result<()> {
    if let Some(target) = cargo.target_selector() {
        return Err(CliError::CommandFailed {
            command: format!("pack c is host-only; remove cargo --target '{target}'"),
            status: None,
        });
    }
    if let Some(target) = cargo.configured_build_target() {
        return Err(CliError::CommandFailed {
            command: format!("pack c is host-only; remove cargo build.target '{target}'"),
            status: None,
        });
    }
    Ok(())
}

fn ensure_c_library_outputs(expansion: &BindingExpansion) -> Result<()> {
    let library = expansion.selected_library();
    if library.builds_cdylib() && library.builds_staticlib() {
        return Ok(());
    }
    Err(CliError::CommandFailed {
        command:
            "pack c requires the selected Rust library target to build both cdylib and staticlib"
                .to_owned(),
        status: None,
    })
}

fn copy_file(source: PathBuf, dest: PathBuf) -> Result<()> {
    std::fs::copy(&source, &dest)
        .map(|_| ())
        .map_err(|source_err| CliError::CopyFailed {
            from: source,
            to: dest,
            source: source_err,
        })
}

pub(crate) fn pack_c(config: &Config, options: PackCOptions, reporter: &Reporter) -> Result<()> {
    if !config.is_c_enabled() {
        return Err(CliError::CommandFailed {
            command: "targets.c.enabled = false".to_string(),
            status: None,
        });
    }
    if !config.should_process(Target::C, options.experimental) {
        return Err(CliError::CommandFailed {
            command: "c is experimental, use --experimental or add \"c\" to experimental"
                .to_owned(),
            status: None,
        });
    }

    reporter.section("🌐", "Packing C");

    let build_cargo_args = resolve_build_cargo_args(config, &options.execution.cargo_args);
    let cargo = Cargo::current(&build_cargo_args)?;
    ensure_c_pack_cargo_args_supported(&cargo)?;
    let build_profile = resolve_build_profile(options.execution.release, &build_cargo_args);
    let binding_expansion = BindingExpansion::resolve(config, &build_cargo_args)?;
    ensure_c_library_outputs(&binding_expansion)?;

    // Generating the header runs a metadata rustc invocation. Build the final
    // artifacts afterwards so that metadata compilation cannot overwrite the
    // binding-expansion cdylib/staticlib selected for this package.
    if options.execution.regenerate {
        let step = reporter.step("Generating C bindings");
        run_generate_with_output(
            config,
            GenerateOptions {
                target: GenerateTarget::C,
                output: Some(config.c_output()),
                experimental: options.experimental,
                cargo_args: build_cargo_args.clone(),
                deny_skipped: options.execution.deny_skipped,
            },
        )?;
        step.finish_success();
    }

    let platform = NativeHostPlatform::current().ok_or_else(|| CliError::CommandFailed {
        command: "pack c is unsupported on this host platform".to_owned(),
        status: None,
    })?;
    let artifact_name = binding_expansion.artifact_name().to_owned();
    let profile_dir = binding_expansion
        .target_directory()
        .join(build_profile.output_directory_name());

    if !options.execution.no_build {
        let step = reporter.step("Building Rust shared and static libraries");
        let on_output: Option<OutputCallback> = step
            .is_verbose()
            .then(|| Box::new(print_cargo_line) as OutputCallback);
        let builder = Builder::new(
            config,
            BuildOptions {
                release: matches!(build_profile, CargoBuildProfile::Release),
                selection: BuildSelection::Expanded(Box::new(binding_expansion)),
                on_output,
                extra_env: Vec::new(),
            },
        );
        if !builder.build_host()? {
            return Err(PackError::BuildFailed {
                targets: vec![platform.canonical_name().to_owned()],
            }
            .into());
        }
        step.finish_success();
    }

    let step = reporter.step("Packaging header and libraries");

    let output_dir = config.c_output();
    let include_dir = output_dir.join("include");
    let lib_dir = output_dir.join("lib");
    std::fs::create_dir_all(&include_dir).map_err(|source| CliError::CreateDirectoryFailed {
        path: include_dir.clone(),
        source,
    })?;
    std::fs::create_dir_all(&lib_dir).map_err(|source| CliError::CreateDirectoryFailed {
        path: lib_dir.clone(),
        source,
    })?;

    let library_name = config.library_name().to_string();

    // Header.
    let header_source = output_dir.join("boltffi.h");
    let header_dest = include_dir.join(format!("{library_name}.h"));
    copy_file(header_source, header_dest)?;

    // Shared library.
    copy_file(
        profile_dir.join(platform.shared_library_filename(&artifact_name)),
        lib_dir.join(platform.shared_library_filename(&library_name)),
    )?;

    // Static archive.
    copy_file(
        profile_dir.join(platform.static_library_filename(&artifact_name)),
        lib_dir.join(platform.static_library_filename(&library_name)),
    )?;

    if let (Some(source_name), Some(destination_name)) = (
        platform.import_library_filename(&artifact_name),
        platform.import_library_filename(&library_name),
    ) {
        copy_file(
            profile_dir.join(source_name),
            lib_dir.join(destination_name),
        )?;
    }

    step.finish_success();
    reporter.finish();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_c_library_outputs, pack_c};
    use crate::build::BindingExpansion;
    use crate::commands::pack::{PackCOptions, PackExecutionOptions};
    use crate::config::Config;
    use crate::reporter::{Reporter, Verbosity};

    #[test]
    fn c_pack_requires_opt_in_even_without_regeneration_or_build() {
        let config: Config =
            toml::from_str("[package]\nname = \"demo\"\n[targets.c]\nenabled = true\n")
                .expect("config");
        let options = PackCOptions {
            execution: PackExecutionOptions {
                release: false,
                regenerate: false,
                no_build: true,
                deny_skipped: false,
                cargo_args: Vec::new(),
            },
            experimental: false,
        };
        let error = pack_c(&config, options, &Reporter::new(Verbosity::Quiet))
            .expect_err("C packaging requires opt-in before any Cargo work");
        assert!(error.to_string().contains("c is experimental"));
    }

    #[test]
    fn c_pack_requires_shared_and_static_library_outputs() {
        for (staticlib, cdylib) in [(false, true), (true, false), (false, false)] {
            let expansion = BindingExpansion::fixture(
                "/external/workspace/Cargo.toml",
                "/external/workspace/demo/Cargo.toml",
                std::iter::empty(),
            )
            .fixture_outputs(staticlib, cdylib);
            let error = ensure_c_library_outputs(&expansion).expect_err("missing output rejects");
            assert!(format!("{error}").contains("both cdylib and staticlib"));
        }
    }
}
