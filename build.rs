use anyhow::{anyhow, bail, Context};
use std::path::{Path, PathBuf};

fn find_or_tools_homebrew() -> anyhow::Result<Option<PathBuf>> {
    const OR_TOOLS_HOMEBREW_DIR: [&str; 2] = [
        "/opt/homebrew/opt/or-tools",
        "/opt/homebrew/opt/or-tools@9.14",
    ];
    const OR_TOOLS_HOMEBREW_INCLUDE_DIR: &str = "/opt/homebrew/include";
    for path in OR_TOOLS_HOMEBREW_DIR.iter() {
        if std::fs::exists(path)
            .context("Failed to check if homebrew libortools directory exists")?
        {
            return Ok(Some(PathBuf::from(OR_TOOLS_HOMEBREW_INCLUDE_DIR)));
        }
    }

    Ok(None)
}

fn find_or_tools_linux() -> anyhow::Result<Option<PathBuf>> {
    // Since OR Tools does not provide a pkg-config integration, we are left finding it manually
    // using heuristics. OR Tools has the lib inside the lib64 directory.
    const OR_TOOLS_LIB_PATHS: [&str; 6] = [
        "/usr/local/lib64",
        "/usr/local/lib",
        "/usr/lib64",
        "/usr/lib",
        "/lib",
        "/lib64",
    ];

    let mut lib_found = false;
    for path in OR_TOOLS_LIB_PATHS.iter() {
        let lib_path = Path::new(path).join("libortools.so");
        if std::fs::exists(lib_path).context("Failed to check if libortools exists")? {
            println!("cargo:rustc-link-search=native={path}");
            lib_found = true;
            break;
        }
    }

    if !lib_found {
        return Ok(None);
    }

    const INCLUDE_PATHS: [&str; 2] = ["/usr/local/include", "/usr/include"];
    for path in INCLUDE_PATHS.iter() {
        if std::fs::exists(Path::new(path).join("ortools"))
            .context("Failed to check if include dir exists")?
        {
            return Ok(Some(PathBuf::from(path)));
        }
    }

    Ok(None)
}

/// Finds the OR Tools library directory, adds it to the linker search path, and returns the include
/// directory.
fn find_or_tools(target: &str) -> anyhow::Result<PathBuf> {
    println!("cargo:rerun-if-env-changed=OR_TOOLS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=OR_TOOLS_INCLUDE_DIR");
    let custom_lib_dir = if let Some(lib_dir) = std::env::var("OR_TOOLS_LIB_DIR").ok() {
        println!("cargo:rustc-link-search=native={lib_dir}");
        true
    } else {
        false
    };
    let custom_include_dir = std::env::var("OR_TOOLS_INCLUDE_DIR").ok();

    // In case both the include dir and the lib dir are already set, we don't need to do anything.
    if custom_lib_dir && custom_include_dir.is_some() {
        return Ok(PathBuf::from(
            custom_include_dir.expect("include dir should be set"),
        ));
    } else if custom_lib_dir || custom_include_dir.is_some() {
        println!(
            "cargo::error='OR_TOOLS_LIB_DIR' and 'OR_TOOLS_INCLUDE_DIR' must be set together."
        );
        bail!("'OR_TOOLS_LIB_DIR' and 'OR_TOOLS_INCLUDE_DIR' must be set together.");
    }

    if target.ends_with("-apple-darwin") {
        match target {
            "aarch64-apple-darwin" => {
                if let Some(include_dir) = find_or_tools_homebrew()
                    .context("Failed to check if homebrew libortools directory exists")?
                {
                    println!("cargo:rustc-link-search=/opt/homebrew/lib");
                    return Ok(include_dir);
                }
            }
            _ => bail!("Unsupported Apple platform: {}", target),
        }

        println!("cargo::error=Could not find `libortools` library. Run `brew install or-tools` or provide the `OR_TOOLS_LIB_DIR` env var.");
        bail!("Could not find `libortools` library");
    }

    if target.contains("unknown-linux-gnu") {
        if let Some(include_path) =
            find_or_tools_linux().context("Failed to check if libortools exists on Linux target")?
        {
            return Ok(include_path);
        }

        println!("cargo::error=Could not find `libortools` library. If not installed in a standard location provide the `OR_TOOLS_LIB_DIR` env var.");
        bail!("Could not find `libortools` library");
    }

    println!("cargo::error=Unsupported platform: {}. Alternatively provide the `OR_TOOLS_LIB_DIR` env variable.", target);
    Err(anyhow!("Unsupported platform: {}", target))
}

fn main() -> anyhow::Result<()> {
    let target = std::env::var("TARGET").expect("TARGET env var is not set");
    let host = std::env::var("HOST").expect("HOST env var is not set");
    if target != host {
        println!("cargo::error=Cross-compilation is currently not supported.?");
        bail!("Cross-compilation is not supported")
    }

    prost_build::compile_protos(
        &["src/cp_model.proto", "src/sat_parameters.proto"],
        &["src/"],
    )
    .context("Failed to compile proto files")?;

    let include_dir = find_or_tools(&target)?;

    if std::env::var("DOCS_RS").is_err() {
        println!("cargo:rerun-if-changed=src/cp_sat_wrapper.cpp");
        cc::Build::new()
            .cpp(true)
            .flags(["-std=c++17", "-DOR_PROTO_DLL="])
            .file("src/cp_sat_wrapper.cpp")
            .include(&include_dir)
            .compile("cp_sat_wrapper.a");
    }

    println!("cargo:rustc-link-lib=ortools");
    println!("cargo:rustc-link-lib=protobuf");

    Ok(())
}
