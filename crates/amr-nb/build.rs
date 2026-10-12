use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=J2PLAY_ANDROID_AMR_ROOT");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        let root = env::var_os("J2PLAY_ANDROID_AMR_ROOT")
            .expect("build Android through dev.sh to prepare OpenCORE AMR for each ABI");
        let target = env::var("TARGET").expect("Cargo supplies TARGET");
        let directory = PathBuf::from(root).join(target).join("lib");
        assert!(
            directory.join("libopencore-amrnb.a").is_file(),
            "missing Android OpenCORE AMR library: {}",
            directory.display()
        );
        println!("cargo:rustc-link-search=native={}", directory.display());
    }
}
