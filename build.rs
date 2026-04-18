fn main() {
    #[cfg(windows)]
    {
        // Clap + the full command graph can exceed the default 1 MiB Windows
        // main-thread stack during process startup. Reserve a larger stack for
        // the CLI binary so `munin.exe --version` and `--help` start reliably.
        println!("cargo:rustc-link-arg=/STACK:8388608");
    }
}
