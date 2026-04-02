fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let datetime = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_DATETIME={datetime}");
}
