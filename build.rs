use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BUILD_TIMESTAMP");

    let utc = std::env::var("BUILD_TIMESTAMP_UTC")
        .unwrap_or_else(|_| date_for_timezone("UTC", "%Y-%m-%dT%H:%M:%SZ"));
    let pacific = std::env::var("BUILD_TIMESTAMP_PACIFIC")
        .unwrap_or_else(|_| date_for_timezone("America/Los_Angeles", "%B %-d, %Y at %-I:%M %p %Z"));

    println!("cargo:rustc-env=BUILD_TIMESTAMP_UTC={utc}");
    println!("cargo:rustc-env=BUILD_TIMESTAMP_PACIFIC={pacific}");
}

fn date_for_timezone(timezone: &str, format: &str) -> String {
    Command::new("date")
        .env("TZ", timezone)
        .arg(format!("+{format}"))
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
