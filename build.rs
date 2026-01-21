use os_info::Type;

// Precompiled binaries for OR-TOOLS
const DEBIAN_12_BINARIES: &str = "https://github.com/google/or-tools/releases/download/v9.14/or-tools_amd64_debian-12_cpp_v9.14.6206.tar.gz";
const ARCH_BINARIES: &str = "https://github.com/google/or-tools/releases/download/v9.14/or-tools_amd64_archlinux_cpp_v9.14.6206.tar.gz";
const MACOS_ARM_BINARIES: &str = "https://github.com/google/or-tools/releases/download/v9.14/or-tools_arm64_macOS-15.5_cpp_v9.14.6206.tar.gz";

/// Helper for printing during build.
macro_rules! warn_print {
    ($($tokens: tt)*) => {
        println!("cargo::warning={}", format!($($tokens)*))
    }
}

fn download_and_extract_binaries(
    platform: Type,
    outpath: std::path::PathBuf,
) -> anyhow::Result<()> {
    let prebuilt_binaries_link = match platform {
        // Arch and "derivative" distros
        Type::Arch | Type::Manjaro | Type::EndeavourOS => ARCH_BINARIES,
        Type::Debian | Type::Linux => DEBIAN_12_BINARIES,
        Type::Macos => MACOS_ARM_BINARIES,
        other => unimplemented!("support for {other} operating system is not implemented"),
    };

    // Download the binaries
    let mut body = reqwest::blocking::get(prebuilt_binaries_link)?;
    let work_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let tar_path = work_dir.join("ortools.tar.gz");
    let mut tar_gz_file = std::fs::File::create(&tar_path)?;
    std::io::copy(&mut body, &mut tar_gz_file)?;

    // Extract the archive
    let decoder = flate2::read::GzDecoder::new(std::fs::File::open(&tar_path)?);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(&work_dir)?;
    std::fs::remove_file(&tar_path)?;

    // Rename extraction dir to a consistent name across different platforms.
    let extraction_dir_regex = regex::Regex::new(r"or.*?tools.*?9\.14").unwrap();
    let extracted_dir = std::fs::read_dir(&work_dir)?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            (entry.path().is_dir()
                && extraction_dir_regex
                    .is_match(entry.path().to_str().expect("should be valid unicode")))
            .then_some(entry.path())
        })
        .ok_or(anyhow::anyhow!("cannot find extracted libs"))?;
    std::fs::rename(&extracted_dir, &outpath)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    prost_build::compile_protos(
        &["src/cp_model.proto", "src/sat_parameters.proto"],
        &["src/"],
    )
    .unwrap();

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let lib_dir = out_dir.join("ortools");
    warn_print!("installing ortools lib in {lib_dir:?}");
    let platform = os_info::get().os_type();

    if platform == os_info::Type::Macos {
        // dynamic linking with the OUT_DIR path of the library does not work for some reason
        // on MacOS, when `cp_sat` is used as a dependency. In this case we rely on system
        // libraries of protobuf & or-tools
        let libortools = "/opt/homebrew/lib/libortools.9.dylib";
        if !std::fs::exists(libortools)? {
            println!("cargo::error=Could not find required `{libortools}`. Did you `brew install or-tools`?");
            return Ok(());
        }
        // No error, `cp_sat` should work with system dependencies
    }
    // Add the precompiled binaries to the OUT_DIR for linking against C++ wrapper.
    if !lib_dir.exists() {
        download_and_extract_binaries(platform, lib_dir.clone())?;
    }

    if std::env::var("DOCS_RS").is_err() {
        let lib_dir_str = lib_dir.to_str().expect("path should be a valid str");
        cc::Build::new()
            .cpp(true)
            .flags(["-std=c++17", "-DOR_PROTO_DLL="])
            .file("src/cp_sat_wrapper.cpp")
            .include([lib_dir_str, "/include"].concat())
            .compile("cp_sat_wrapper.a");

        println!("cargo:rustc-link-lib=ortools");
        println!("cargo:rustc-link-lib=protobuf");
        match platform {
            os_info::Type::Macos => println!("cargo:rustc-link-search=/opt/homebrew/lib"),
            _other => println!("cargo:rustc-link-search={}/lib", lib_dir_str),
        }
    }
    Ok(())
}
