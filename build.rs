// Bake a version string into the binary.
//
// Release builds (CI on a version tag) pass STARS_VERSION so the binary,
// container image, and Helm chart all report the same X.Y.Z. Local and
// branch builds fall back to the Cargo.toml version.
fn main() {
    println!("cargo:rerun-if-env-changed=STARS_VERSION");
    let version = std::env::var("STARS_VERSION")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("CARGO_PKG_VERSION").ok())
        .unwrap_or_else(|| "0.0.0".to_string());
    println!("cargo:rustc-env=STARS_VERSION={version}");
}
