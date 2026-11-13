use crate::util;
use anyhow::{bail, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn ensure_termux() -> Result<()> {
    let prefix = util::prefix();
    if !prefix.starts_with("/data/data/com.termux") {
        bail!(
            "esto no parece Termux (PREFIX={}). Ejecuta la herramienta dentro de Termux.",
            prefix.display()
        );
    }
    if util::which("pkg").is_none() {
        bail!("no se encontró el comando `pkg` de Termux");
    }
    Ok(())
}

pub fn arch() -> String {
    util::quiet("uname", &["-m"])
        .unwrap_or_else(|_| "desconocida".into())
        .trim()
        .to_string()
}

pub fn is_arm64() -> bool {
    matches!(arch().as_str(), "aarch64" | "arm64")
}

pub fn install_packages(packages: &[&str]) -> Result<()> {
    println!("  Actualizando índices de paquetes...");
    // Si falla (sin red, mirror caído) se intenta instalar igualmente.
    let _ = util::output("pkg", &["update", "-y"]);

    let mut args = vec!["install", "-y"];
    args.extend_from_slice(packages);
    println!("  pkg {}", args.join(" "));
    util::run("pkg", &args)
}

pub fn lan_ip() -> Option<String> {
    let out = util::quiet("ifconfig", &[]).ok()?;
    for line in out.lines() {
        if let Some(rest) = line.trim().strip_prefix("inet ") {
            if let Some(ip) = rest.split_whitespace().next() {
                if ip != "127.0.0.1" && !ip.starts_with("0.") && !ip.contains(':') {
                    return Some(ip.to_string());
                }
            }
        }
    }
    None
}

pub fn java_version() -> Option<String> {
    let out = util::output("java", &["-version"]).ok()?;
    let mut text = String::from_utf8_lossy(&out.stderr).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    text.lines()
        .find(|line| line.contains("version"))
        .map(|line| line.trim().to_string())
}

pub fn mem_available_mb() -> Option<u64> {
    let raw = fs::read_to_string("/proc/meminfo").ok()?;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

pub fn free_disk_mb(path: &Path) -> Option<u64> {
    let out = util::quiet("df", &["-k", &path.to_string_lossy()]).ok()?;
    let line = out.lines().last()?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    // Filesystem 1K-blocks Used Available Capacity Mounted
    fields.get(3)?.parse::<u64>().ok().map(|kb| kb / 1024)
}

pub fn app_installed(package: &str) -> bool {
    let expected = format!("package:{package}");
    Command::new("pm")
        .args(["list", "packages"])
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|line| line.trim() == expected)
        })
        .unwrap_or(false)
}
