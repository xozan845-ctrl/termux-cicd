use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data/data/com.termux/files/home"))
}

pub fn prefix() -> PathBuf {
    env::var_os("PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data/data/com.termux/files/usr"))
}

pub fn expand_home(path: &Path) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~") {
        home().join(rest)
    } else {
        path.to_path_buf()
    }
}

pub fn which(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let p = PathBuf::from(program);
        return p.is_file().then_some(p);
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

/// Ejecuta un comando heredando stdout/stderr (para procesos interactivos).
pub fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("no se pudo ejecutar {program}"))?;
    if !status.success() {
        bail!("`{program} {}` terminó con {status}", args.join(" "));
    }
    Ok(())
}

pub fn output(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("no se pudo ejecutar {program}"))
}

/// Ejecuta y devuelve stdout, fallando si el comando falla.
pub fn quiet(program: &str, args: &[&str]) -> Result<String> {
    let out = output(program, args)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!(
            "`{program} {}` falló: {}",
            args.join(" "),
            err.trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Como `run`, pero acepta ciertos códigos de salida (p. ej. tar=1 cuando un
/// archivo cambia mientras se lee) tratándolos como advertencia.
pub fn run_tolerant(program: &str, args: &[&str], allowed_codes: &[i32]) -> Result<()> {
    let out = output(program, args)?;
    if out.status.success() {
        return Ok(());
    }
    let code = out.status.code().unwrap_or(-1);
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if allowed_codes.contains(&code) {
        if !err.is_empty() {
            eprintln!("  Aviso: {err}");
        }
        return Ok(());
    }
    bail!("`{program} {}` falló (código {code}): {err}", args.join(" "));
}

pub fn download(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    let _ = fs::remove_file(&tmp);
    let status = Command::new("curl")
        .arg("-fL")
        .arg("--retry")
        .arg("3")
        .arg("--connect-timeout")
        .arg("30")
        .arg("-o")
        .arg(&tmp)
        .arg(url)
        .status()
        .context("no se pudo ejecutar curl")?;
    if !status.success() {
        let _ = fs::remove_file(&tmp);
        bail!("la descarga de {url} falló");
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn set_executable(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

pub fn write_private(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

pub fn random_password(len: usize) -> Result<String> {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut bytes = vec![0u8; len];
    fs::File::open("/dev/urandom")
        .context("no se pudo abrir /dev/urandom")?
        .read_exact(&mut bytes)?;
    Ok(bytes
        .iter()
        .map(|b| CHARS[*b as usize % CHARS.len()] as char)
        .collect())
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn dir_size(path: &Path) -> u64 {
    fn walk(p: &Path, total: &mut u64) {
        if let Ok(meta) = fs::symlink_metadata(p) {
            if meta.is_dir() {
                if let Ok(entries) = fs::read_dir(p) {
                    for entry in entries.flatten() {
                        walk(&entry.path(), total);
                    }
                }
            } else {
                *total += meta.len();
            }
        }
    }
    let mut total = 0;
    walk(path, &mut total);
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genera_password_con_longitud_y_alfabeto() {
        let password = random_password(24).expect("urandom");
        assert_eq!(password.chars().count(), 24);
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn formatea_tamanos_legibles() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
