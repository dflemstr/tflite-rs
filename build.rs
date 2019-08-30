extern crate bindgen;
extern crate cpp_build;
extern crate curl;
extern crate failure;
extern crate flate2;
extern crate hex;
extern crate reqwest;
extern crate sha2;
extern crate tar;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use failure::Fallible;

const TFLITE_VERSION: &'static str = "1.13.2";

fn is_valid_tf_src<P: AsRef<Path>>(filepath: P) -> bool {
    use sha2::{Digest, Sha256};

    let mut sha256 = Sha256::new();
    sha256.input(&fs::read(filepath).unwrap());
    ::hex::encode(sha256.result())
        == "abe3bf0c47845a628b7df4c57646f41a10ee70f914f1b018a5c761be75e1f1a9"
}

fn download<P: reqwest::IntoUrl, P2: AsRef<Path>>(source_url: P, target_file: P2) -> Fallible<()> {
    let mut resp = reqwest::get(source_url)?;
    let f = fs::File::create(&target_file)?;
    let mut writer = ::std::io::BufWriter::new(f);
    resp.copy_to(&mut writer)?;
    Ok(())
}

fn extract<P: AsRef<Path>, P2: AsRef<Path>>(archive_path: P, extract_to: P2) {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = fs::File::open(archive_path).unwrap();
    let unzipped = GzDecoder::new(file);
    let mut a = Archive::new(unzipped);
    a.unpack(extract_to).unwrap();
}

fn prepare_tensorflow_source() -> PathBuf {
    let tf_src_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("tensorflow");
    fs::create_dir(&tf_src_dir).unwrap_or_default();

    let tf_src_name = tf_src_dir.join(format!("v{}.tar.gz", TFLITE_VERSION));
    if !tf_src_name.exists() || !is_valid_tf_src(&tf_src_name) {
        let tf_src_url = format!(
            "https://codeload.github.com/tensorflow/tensorflow/tar.gz/v{}",
            TFLITE_VERSION
        );

        download(&tf_src_url, &tf_src_name).unwrap();

        assert!(
            is_valid_tf_src(&tf_src_name),
            "{} is not valid",
            tf_src_name.to_str().unwrap()
        );
    }

    let tf_src_dir_inner = tf_src_dir.join(format!("tensorflow-{}", TFLITE_VERSION));
    if !tf_src_dir_inner.exists() {
        extract(&tf_src_name, &tf_src_dir);

        Command::new("tensorflow/lite/tools/make/download_dependencies.sh")
            .current_dir(&tf_src_dir_inner)
            .status()
            .expect("failed to download tflite dependencies.");

        // To compile C files with -fPIC
        if env::var("CARGO_CFG_TARGET_OS").unwrap() == "linux" {
            fs::copy(
                "data/linux_makefile.inc",
                tf_src_dir_inner.join("tensorflow/lite/tools/make/targets/linux_makefile.inc"),
            )
            .expect("Unable to copy linux makefile");
        }
        // To allow for cross-compiling to aarch64
        if env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "aarch64" {
            fs::copy(
                "data/aarch64_makefile.inc",
                tf_src_dir_inner.join("tensorflow/lite/tools/make/targets/aarch64_makefile.inc"),
            )
            .expect("Unable to copy aarch64 makefile");
        }

        #[cfg(feature = "debug_tflite")]
        {
            Command::new("sed")
                .arg("-i")
                .arg("54s/.*/CXXFLAGS := -O0 -g -fno-inline/")
                .arg("tensorflow/lite/tools/make/Makefile")
                .current_dir(&tf_src_dir_inner)
                .status()
                .expect("failed to edit Makefile.");

            Command::new("sed")
                .arg("-i")
                .arg("57s/.*/CFLAGS := -O0 -g -fno-inline/")
                .arg("tensorflow/lite/tools/make/Makefile")
                .current_dir(&tf_src_dir_inner)
                .status()
                .expect("failed to edit Makefile.");
        }
    }
    tf_src_dir_inner
}

fn prepare_tensorflow_library<P: AsRef<Path>>(tflite: P) {
    let tf_lib_name = PathBuf::from(env::var("OUT_DIR").unwrap()).join("libtensorflow-lite.a");
    let os = env::var("CARGO_CFG_TARGET_OS").expect("Unable to get TARGET_OS");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Unable to get TARGET_ARCH");
    if !tf_lib_name.exists() {
        Command::new("make")
            .arg("-j")
            // allow parallelism to be overridden
            .arg(env::var("TFLITE_RS_MAKE_PARALLELISM").unwrap_or("3".to_owned()))
            .arg("-f")
            .arg("tensorflow/lite/tools/make/Makefile")
            // Use cargo's cross-compilation information while building tensorflow
            .arg(format!("TARGET={}", os))
            .arg(format!("TARGET_ARCH={}", arch))
            .current_dir(&tflite)
            .status()
            .expect("failed to build tensorflow");

        fs::copy(
            tflite.as_ref().join(format!(
                "tensorflow/lite/tools/make/gen/{OS}_{ARCH}/lib/libtensorflow-lite.a",
                OS = os,
                ARCH = arch,
            )),
            &tf_lib_name,
        )
        .expect("Unable to copy libtensorflow-lite.a to OUT_DIR");
    }
}

// This generates "tflite_types.rs" containing structs and enums which are inter-operable with Glow.
fn import_tflite_types<P: AsRef<Path>>(tflite: P) {
    use bindgen::*;

    let bindings = Builder::default()
        .whitelist_recursively(true)
        .prepend_enum_name(false)
        .impl_debug(true)
        .with_codegen_config(CodegenConfig::TYPES)
        .layout_tests(false)
        .enable_cxx_namespaces()
        .derive_default(true)
        // for model APIs
        .whitelist_type("tflite::ModelT")
        .whitelist_type(".+OptionsT")
        .blacklist_type(".+_TableType")
        // for interpreter
        .whitelist_type("tflite::FlatBufferModel")
        .opaque_type("tflite::FlatBufferModel")
        .whitelist_type("tflite::InterpreterBuilder")
        .opaque_type("tflite::InterpreterBuilder")
        .whitelist_type("tflite::Interpreter")
        .opaque_type("tflite::Interpreter")
        .whitelist_type("tflite::ops::builtin::BuiltinOpResolver")
        .opaque_type("tflite::ops::builtin::BuiltinOpResolver")
        .whitelist_type("tflite::OpResolver")
        .opaque_type("tflite::OpResolver")
        .whitelist_type("TfLiteTensor")
        .blacklist_type("std")
        .blacklist_type("tflite::Interpreter_TfLiteDelegatePtr")
        .blacklist_type("tflite::Interpreter_State")
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .header("csrc/tflite_wrapper.hpp")
        .clang_arg(format!("-I{}", tflite.as_ref().to_str().unwrap()))
        .clang_arg(format!(
            "-I{}/tensorflow/lite/tools/make/downloads/flatbuffers/include",
            tflite.as_ref().to_str().unwrap()
        ))
        .clang_arg("-DGEMMLOWP_ALLOW_SLOW_SCALAR_FALLBACK")
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++11")
        // required to get cross compilation for aarch64 to work because of an issue in flatbuffers
        .clang_arg("-fms-extensions");

    let bindings = bindings.generate().expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/tflite_types.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("tflite_types.rs");
    bindings
        .write_to_file(out_path)
        .expect("Couldn't write bindings!");
}

fn build_inline_cpp<P: AsRef<Path>>(tflite: P) {
    println!("cargo:rustc-link-lib=static=tensorflow-lite");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");

    cpp_build::Config::new()
        .include(tflite.as_ref())
        .include(
            tflite
                .as_ref()
                .join("tensorflow/lite/tools/make/downloads/flatbuffers/include"),
        )
        .flag("-fPIC")
        .flag("-std=c++11")
        .flag("-Wno-sign-compare")
        .define("GEMMLOWP_ALLOW_SLOW_SCALAR_FALLBACK", None)
        .debug(true)
        .opt_level(if cfg!(debug_assertions) { 0 } else { 2 })
        .build("src/lib.rs");
}

fn main() {
    let tflite_src_dir = prepare_tensorflow_source();
    prepare_tensorflow_library(&tflite_src_dir);
    import_tflite_types(&tflite_src_dir);
    build_inline_cpp(&tflite_src_dir);
}
