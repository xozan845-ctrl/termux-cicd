use crate::config::{Config, Paths};
use crate::util;
use anyhow::{bail, Context, Result};
use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const PLUGIN_MANAGER_VERSION: &str = "2.16.0";
pub const PLUGINS: &[&str] = &[
    "configuration-as-code",
    "git",
    "github",
    "github-branch-source",
    "workflow-aggregator",
    "pipeline-stage-view",
    "credentials-binding",
    "timestamper",
    "ws-cleanup",
    "matrix-auth",
    "ansicolor",
    "dark-theme",
];

const INIT_TEMPLATE: &str = include_str!("../templates/init-security.groovy");

pub fn latest_version() -> Result<String> {
    let out = util::quiet(
        "curl",
        &[
            "-fsSL",
            "--connect-timeout",
            "30",
            "https://updates.jenkins.io/stable/latestCore.txt",
        ],
    )?;
    let version = out.trim().to_string();
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        bail!("respuesta inesperada al consultar la versión LTS: {version:?}");
    }
    Ok(version)
}

pub fn installed_version(p: &Paths) -> Option<String> {
    fs::read_to_string(p.version_file())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn ensure_password(p: &Paths) -> Result<(String, bool)> {
    if p.password().exists() {
        let password = fs::read_to_string(p.password())?.trim().to_string();
        if !password.is_empty() {
            return Ok((password, false));
        }
    }
    fs::create_dir_all(p.home())?;
    let password = util::random_password(24)?;
    util::write_private(&p.password(), &format!("{password}\n"))?;
    Ok((password, true))
}

pub fn download_war(p: &Paths, version: &str, force: bool) -> Result<bool> {
    if !force && p.war().exists() && installed_version(p).as_deref() == Some(version) {
        return Ok(false);
    }
    fs::create_dir_all(&p.root)?;
    let url = format!("https://get.jenkins.io/war-stable/{version}/jenkins.war");
    println!("  Descargando {url}");
    util::download(&url, &p.war())?;
    fs::write(p.version_file(), format!("{version}\n"))?;
    Ok(true)
}

pub fn plugins_installed(p: &Paths) -> bool {
    [
        "workflow-aggregator.jpi",
        "git.jpi",
        "github.jpi",
        "configuration-as-code.jpi",
    ]
    .iter()
    .all(|name| p.plugins().join(name).exists())
}

pub fn install_plugins(p: &Paths, force: bool) -> Result<()> {
    if !force && plugins_installed(p) {
        return Ok(());
    }
    fs::create_dir_all(p.plugins())?;

    let mut list = String::new();
    for plugin in PLUGINS {
        list.push_str(plugin);
        list.push('\n');
    }
    fs::write(p.plugins_txt(), list)?;

    if !p.jpm().exists() {
        let url = format!(
            "https://github.com/jenkinsci/plugin-installation-manager-tool/releases/download/{v}/jenkins-plugin-manager-{v}.jar",
            v = PLUGIN_MANAGER_VERSION
        );
        println!("  Descargando jenkins-plugin-manager {PLUGIN_MANAGER_VERSION}");
        util::download(&url, &p.jpm())?;
    }

    println!(
        "  Instalando {} plugins (puede tardar unos minutos)...",
        PLUGINS.len()
    );
    let jpm = p.jpm().to_string_lossy().to_string();
    let war = p.war().to_string_lossy().to_string();
    let plugins_txt = p.plugins_txt().to_string_lossy().to_string();
    let plugins_dir = p.plugins().to_string_lossy().to_string();
    util::run(
        "java",
        &[
            "-jar",
            &jpm,
            "--war",
            &war,
            "--plugin-file",
            &plugins_txt,
            "--plugin-download-directory",
            &plugins_dir,
        ],
    )
    .context("falló la instalación de plugins")?;
    Ok(())
}

pub fn write_init_script(cfg: &Config) -> Result<()> {
    let p = cfg.paths();
    fs::create_dir_all(p.init_dir())?;
    let password_file = p.password().to_string_lossy().to_string();
    let url_file = p.url_file().to_string_lossy().to_string();
    let script = INIT_TEMPLATE
        .replace("{{EXECUTORS}}", &cfg.executors.to_string())
        .replace("{{PASSWORD_FILE}}", &groovy_escape(&password_file))
        .replace("{{URL_FILE}}", &groovy_escape(&url_file));
    fs::write(p.init_script(), script)?;
    Ok(())
}

pub fn groovy_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Escribe url.txt y devuelve la URL usada.
pub fn write_url(cfg: &Config) -> Result<String> {
    let url = if cfg.url.trim().is_empty() {
        let ip = crate::termux::lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        format!("http://{ip}:{}/", cfg.port)
    } else {
        cfg.url.trim().to_string()
    };
    fs::write(cfg.paths().url_file(), format!("{url}\n"))?;
    Ok(url)
}

/// Espera a que Jenkins responda y autentique con el usuario admin.
pub fn wait_ready(port: u16, password: &str, timeout: Duration) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    let auth = format!("admin:{password}");
    let url = format!("http://127.0.0.1:{port}/api/json");
    loop {
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
        if TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok() {
            let code = util::quiet(
                "curl",
                &[
                    "-s",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{http_code}",
                    "--max-time",
                    "5",
                    "-u",
                    auth.as_str(),
                    url.as_str(),
                ],
            )
            .unwrap_or_default();
            if code.trim() == "200" {
                return Ok(true);
            }
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        sleep(Duration::from_secs(2));
    }
}

pub fn http_code(port: u16, path: &str) -> Option<String> {
    let url = format!("http://127.0.0.1:{port}{path}");
    util::quiet(
        "curl",
        &[
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "3",
            url.as_str(),
        ],
    )
    .ok()
    .map(|code| code.trim().to_string())
}

/// Espera a que la página de login responda 200 (sin autenticar).
pub fn wait_http(port: u16, timeout: Duration) -> Result<bool> {
    let deadline = Instant::now() + timeout;
    loop {
        if http_code(port, "/login").as_deref() == Some("200") {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        sleep(Duration::from_secs(2));
    }
}

/// URL base con la que Jenkins se identifica (la misma de `url.txt`).
/// El CLI de Jenkins rechaza peticiones si el origen no coincide exactamente.
pub fn base_url(cfg: &Config) -> Result<String> {
    let file = cfg.paths().url_file();
    let url = if file.exists() {
        fs::read_to_string(&file)
            .with_context(|| format!("no se pudo leer {}", file.display()))?
            .trim()
            .to_string()
    } else {
        let ip = crate::termux::lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        format!("http://{ip}:{}/", cfg.port)
    };
    Ok(url.trim_end_matches('/').to_string())
}

/// Descarga el cliente `jenkins-cli.jar` si todavía no existe.
pub fn ensure_cli_jar(cfg: &Config) -> Result<PathBuf> {
    let p = cfg.paths();
    let jar = p.cli_jar();
    if jar.exists() {
        return Ok(jar);
    }
    let url = format!("{}/jnlpJars/jenkins-cli.jar", base_url(cfg)?);
    println!("  Descargando jenkins-cli.jar...");
    util::download(&url, &jar)?;
    Ok(jar)
}

/// Ejecuta un script Groovy en el servidor mediante el CLI de Jenkins.
/// La contraseña va en un archivo temporal `0600` para no exponerla en `ps`.
pub fn run_groovy(cfg: &Config, script: &str) -> Result<()> {
    let p = cfg.paths();
    let jar = ensure_cli_jar(cfg)?;
    let password = fs::read_to_string(p.password())
        .with_context(|| format!("no se pudo leer {}", p.password().display()))?
        .trim()
        .to_string();
    if password.is_empty() {
        bail!("la contraseña de admin está vacía en {}", p.password().display());
    }

    let auth_file = p.root.join(".cli-auth.tmp");
    util::write_private(&auth_file, &format!("admin:{password}\n"))?;

    let url = base_url(cfg)?;
    let auth_arg = format!("@{}", auth_file.to_string_lossy());
    let result = util::run_with_stdin(
        "java",
        &["-jar", &jar.to_string_lossy(), "-s", &url, "-auth", &auth_arg, "groovy", "="],
        script,
    );
    let _ = fs::remove_file(&auth_file);
    result
}
