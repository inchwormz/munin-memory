use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // Clap + the full command graph can exceed the default 1 MiB Windows
    // main-thread stack during process startup. Reserve a larger stack for
    // the CLI binary so `munin.exe --version` and `--help` start reliably.
    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => println!("cargo:rustc-link-arg=/STACK:8388608"),
        Ok("gnu") => println!("cargo:rustc-link-arg=-Wl,--stack,8388608"),
        Ok(target_env) => panic!("unsupported Windows target environment `{target_env}`"),
        Err(error) => panic!("Cargo did not provide CARGO_CFG_TARGET_ENV: {error}"),
    }
}
