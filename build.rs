#![deny(rust_2018_idioms)]

#[cfg(feature = "generate_binding")]
use std::path::PathBuf;
use std::{env, fmt::Display, path::Path};

/// Outputs the library-file's prefix as word usable for actual arguments on
/// commands or paths.
const fn rustc_linking_word(is_static_link: bool) -> &'static str {
    if is_static_link { "static" } else { "dylib" }
}

/// Generates a new binding at `src/lib.rs` using `src/wrapper.h`.
#[cfg(feature = "generate_binding")]
fn generate_binding() {
    const ALLOW_UNCONVENTIONALS: &'static str = "#![allow(non_upper_case_globals)]\n\
                                                 #![allow(non_camel_case_types)]\n\
                                                 #![allow(non_snake_case)]\n";

    let bindings = bindgen::Builder::default()
        .header("src/wrapper.h")
        .raw_line(ALLOW_UNCONVENTIONALS)
        .generate()
        .expect("Unable to generate binding");

    let binding_target_path = PathBuf::new().join("src").join("lib.rs");

    bindings
        .write_to_file(binding_target_path)
        .expect("Could not write binding to the file at `src/lib.rs`");

    println!("cargo:info=Successfully generated binding.");
}

fn build_opus(is_static: bool) {
    let opus_path = Path::new("opus");

    println!(
        "cargo:info=Opus source path used: {:?}.",
        opus_path
            .canonicalize()
            .expect("Could not canonicalise to absolute path")
    );

    let mut dst = cmake::Config::new(opus_path);

    #[cfg(target_os = "android")]
    {
        println!("cargo:rerun-if-env-changed=ANDROID_NDK");
        if let Ok(ndk) = std::env::var("ANDROID_NDK") {
            dst.define("CMAKE_SYSTEM_NAME", "Android");
            dst.define("ANDROID_NDK", ndk.clone());
            dst.define("CMAKE_ANDROID_NDK", ndk);
            dst.define("__ANDROID_API__", "24");
        }

        println!("cargo:rerun-if-env-changed=ANDROID_ABI");
        if let Ok(abi) = std::env::var("ANDROID_ABI") {
            dst.define("ANDROID_ABI", abi);
        }
    }

    println!("cargo:rerun-if-env-changed=CMAKE_TOOLCHAIN_FILE");
    if let Ok(toolchain) = std::env::var("CMAKE_TOOLCHAIN_FILE") {
        dst.define("CMAKE_TOOLCHAIN_FILE", toolchain);
    }

    println!("cargo:rerun-if-env-changed=CMAKE_SYSTEM_PROCESSOR");
    if let Ok(abi) = std::env::var("CARGO_CFG_TARGET_ARCH") {
        dst.define("CMAKE_SYSTEM_PROCESSOR", map_architecture(&abi));
    }

    if env::var("CARGO_FEATURE_QEXT").is_ok() {
        println!("cargo:info=Enabling QEXT.");
        dst.define("OPUS_QEXT", "ON");
    }
    if env::var("CARGO_FEATURE_DRED").is_ok() {
        println!("cargo:info=Enabling DRED.");
        dst.define("OPUS_DRED", "ON");
    }
    if env::var("CARGO_FEATURE_OSCE").is_ok() {
        println!("cargo:info=Enabling OSCE.");
        dst.define("OPUS_OSCE", "ON");
    }
    if env::var("CARGO_FEATURE_DISABLE_ENCODER").is_ok() {
        println!("cargo:info=Disabling Encoder.");
        dst.define("OPUS_DISABLE_ENCODER", "ON");
    }
    if env::var("CARGO_FEATURE_DISABLE_DECODER").is_ok() {
        println!("cargo:info=Disabling Decoder.");
        dst.define("OPUS_DISABLE_DECODER", "ON");
    }

    println!("cargo:info=Building Opus via CMake.");
    let opus_build_dir = dst.build();
    link_opus(is_static, opus_build_dir.display())
}

fn link_opus(is_static: bool, opus_build_dir: impl Display) {
    let is_static_text = rustc_linking_word(is_static);

    println!(
        "cargo:info=Linking Opus as {} lib: {}",
        is_static_text, opus_build_dir
    );
    println!("cargo:rustc-link-lib={}=opus", is_static_text);
    println!("cargo:rustc-link-search=native={}/lib", opus_build_dir);
}

fn map_architecture(arch: &str) -> &str {
    match arch {
        "arm" => "armv7-a",
        "aarch64" => "aarch64",
        _ => arch,
    }
}

/// Based on the OS or target environment we are building for,
/// this function will return an expected default library linking method.
///
/// If we build for Windows, MacOS, or Linux with musl, we will link statically.
/// However, if you build for Linux without musl, we will link dynamically.
///
/// **Info**:
/// This is a helper-function and may not be called if
/// if the `static`-feature is enabled, the environment variable
/// `LIBOPUS_STATIC` or `OPUS_STATIC` is set.
fn default_library_linking() -> bool {
    #[cfg(any(windows, target_os = "macos", target_env = "musl"))]
    {
        true
    }
    #[cfg(any(target_os = "freebsd", all(unix, target_env = "gnu")))]
    {
        false
    }
}

fn is_static_build() -> bool {
    let feature_static = env::var("CARGO_FEATURE_STATIC").is_ok();
    let feature_dynamic = env::var("CARGO_FEATURE_DYNAMIC").is_ok();

    if feature_static && feature_dynamic {
        default_library_linking()
    } else if feature_static
        || env::var("LIBOPUS_STATIC").is_ok()
        || env::var("OPUS_STATIC").is_ok()
    {
        println!("cargo:info=Static feature or environment variable found.");

        true
    } else if feature_dynamic {
        println!("cargo:info=Dynamic feature enabled.");

        false
    } else {
        println!("cargo:info=No feature or environment variable found, linking by default.");

        default_library_linking()
    }
}

fn main() {
    #[cfg(feature = "generate_binding")]
    generate_binding();

    let is_static = is_static_build();
    build_opus(is_static);
}
