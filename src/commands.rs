use crate::cli::{ConfigAction, GithubAction};
use crate::config::{self, Config};
use crate::jenkins::{self, PLUGINS};
use crate::service;
use crate::termux;
use crate::util;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

const GITHUB_JOB_TEMPLATE: &str = include_str!("../templates/github-job.groovy");
const GITHUB_LIST_TEMPLATE: &str = include_str!("../templates/github-list.groovy");

pub fn install(cfg: &mut Config, version: Option<String>, force_plugins: bool) -> Result<()> {
    println!("termux-cicd: instalación de Jenkins\n");

    step(1, 8, "Comprobando entorno Termux");
    termux::ensure_termux()?;
    if !termux::is_arm64() {
        println!(
            "  Aviso: arquitectura {}; la configuración probada es aarch64.",
            termux::arch()
        );
    }

    step(2, 8, "Instalando paquetes del sistema");
    termux::install_packages(&[
        "openjdk-21",
        "git",
        "curl",
        "tar",
        "fontconfig",
        "fontconfig-utils",
        "termux-services",
    ])?;

    step(3, 8, "Preparando directorios");
    let p = cfg.paths();
    for dir in [&p.root, &p.home(), &p.plugins(), &p.init_dir(), &p.logs()] {
        fs::create_dir_all(dir)
            .with_context(|| format!("no se pudo crear {}", dir.display()))?;
    }
    println!("  {}", p.root.display());

    step(4, 8, "Generando credenciales de administrador");
    let (password, created) = jenkins::ensure_password(&p)?;
    println!(
        "  {}",
        if created {
            "contraseña generada"
        } else {
            "contraseña existente reutilizada"
        }
    );

    step(5, 8, "Preparando Jenkins");
    let target = match version {
        Some(version) => version,
        None => {
            println!("  Consultando la última versión LTS...");
            jenkins::latest_version()?
        }
    };
    if jenkins::download_war(&p, &target, false)? {
        println!("  Jenkins {target} descargado");
    } else {
        println!("  Jenkins {target} ya está instalado");
    }

    step(6, 8, "Instalando plugins");
    jenkins::install_plugins(&p, force_plugins)?;
    println!("  {} plugins configurados", PLUGINS.len());

    step(7, 8, "Configurando servicio y arranque automático");
    jenkins::write_init_script(cfg)?;
    let url = jenkins::write_url(cfg)?;
    service::write_files(cfg)?;
    println!("  servicio: {}", service::service_dir().display());

    step(8, 8, "Arrancando Jenkins");
    service::restart()?;
    println!("  esperando respuesta en el puerto {}...", cfg.port);
    let ready = jenkins::wait_ready(cfg.port, &password, Duration::from_secs(120))?;
    if !ready {
        println!("  Aviso: Jenkins aún no responde. Revisa: termux-cicd logs -f");
    }

    cfg.jenkins_version = target;
    cfg.save()?;

    println!();
    print_summary(cfg, &url, &password, ready);
    Ok(())
}

pub fn start(cfg: &Config) -> Result<()> {
    ensure_installed(cfg)?;
    service::start()?;
    let ready = jenkins::wait_http(cfg.port, Duration::from_secs(90))?;
    println!(
        "Jenkins {} en {}",
        if ready { "arrancado" } else { "arrancando" },
        display_url(cfg)
    );
    Ok(())
}

pub fn stop() -> Result<()> {
    service::stop()?;
    println!("Jenkins detenido.");
    Ok(())
}

pub fn restart(cfg: &Config) -> Result<()> {
    ensure_installed(cfg)?;
    service::restart()?;
    let ready = jenkins::wait_http(cfg.port, Duration::from_secs(90))?;
    println!(
        "Jenkins {} en {}",
        if ready { "reiniciado" } else { "reiniciando" },
        display_url(cfg)
    );
    Ok(())
}

pub fn status(cfg: &Config) -> Result<()> {
    let p = cfg.paths();
    if !p.war().exists() {
        println!(
            "Jenkins no está instalado en {}. Ejecuta: termux-cicd install",
            p.root.display()
        );
        return Ok(());
    }

    let st = service::status();
    let version = jenkins::installed_version(&p).unwrap_or_else(|| "desconocida".into());
    println!("Jenkins {}", if st.running { "EN EJECUCIÓN" } else { "DETENIDO" });
    if let Some(pid) = st.pid {
        println!("  PID:          {pid}");
    }
    println!("  Versión:      {version}");
    println!("  URL:          {}", display_url(cfg));
    println!(
        "  HTTP :{}      {}",
        cfg.port,
        jenkins::http_code(cfg.port, "/login").unwrap_or_else(|| "sin respuesta".into())
    );
    println!("  JENKINS_HOME: {}", p.home().display());
    println!("  Datos:        {}", util::human_size(util::dir_size(&p.home())));
    println!("  Admin:        admin (contraseña en {})", p.password().display());
    if !st.running {
        println!("\nArranca con: termux-cicd start");
    }
    Ok(())
}

pub fn logs(cfg: &Config, follow: bool, lines: usize) -> Result<()> {
    service::tail(&cfg.paths(), lines, follow)
}

pub fn update(cfg: &mut Config, version: Option<String>) -> Result<()> {
    let p = cfg.paths();
    ensure_installed(cfg)?;

    let target = match version {
        Some(version) => version,
        None => {
            println!("Consultando la última versión LTS...");
            jenkins::latest_version()?
        }
    };
    let current = jenkins::installed_version(&p).unwrap_or_default();
    if current == target {
        println!("Ya estás en Jenkins {target}. Nada que hacer.");
        return Ok(());
    }

    println!("Actualizando Jenkins {current} -> {target}");
    let new_war = p.root.join("jenkins.war.new");
    let url = format!("https://get.jenkins.io/war-stable/{target}/jenkins.war");
    util::download(&url, &new_war)?;

    service::stop()?;
    let backup = p.root.join(format!("jenkins.war.{current}.bak"));
    if p.war().exists() {
        fs::rename(p.war(), &backup)?;
    }
    fs::rename(&new_war, p.war())?;
    fs::write(p.version_file(), format!("{target}\n"))?;

    cfg.jenkins_version = target.clone();
    cfg.save()?;
    service::start()?;

    let password = fs::read_to_string(p.password())
        .unwrap_or_default()
        .trim()
        .to_string();
    let ready = jenkins::wait_ready(cfg.port, &password, Duration::from_secs(120))?;
    println!(
        "Jenkins {target} {}",
        if ready {
            "listo"
        } else {
            "arrancando (revisa `termux-cicd logs -f`)"
        }
    );
    println!("Copia del war anterior: {}", backup.display());
    Ok(())
}

pub fn backup(cfg: &Config, output: Option<PathBuf>) -> Result<()> {
    let p = cfg.paths();
    if !p.home().exists() {
        bail!("no hay instalación de Jenkins en {}", p.root.display());
    }

    let out = match output {
        Some(path) => util::expand_home(&path),
        None => {
            let stamp = util::quiet("date", &["+%Y-%m-%d-%H%M%S"])?
                .trim()
                .to_string();
            util::home().join(format!("jenkins-backup-{stamp}.tgz"))
        }
    };
    let parent = p.root.parent().context("ruta de instalación inválida")?;
    let name = p
        .root
        .file_name()
        .context("ruta de instalación inválida")?
        .to_string_lossy()
        .to_string();

    let was_running = service::status().running;
    if was_running {
        println!("Deteniendo Jenkins para una copia consistente...");
        service::stop()?;
        if !service::wait_stopped(Duration::from_secs(30)) {
            println!("  Aviso: Jenkins sigue deteniéndose; la copia puede incluir archivos en cambio.");
        }
        sleep(Duration::from_millis(800));
    }

    println!("Creando {}", out.display());
    let out_s = out.to_string_lossy().to_string();
    let parent_s = parent.to_string_lossy().to_string();
    // El war desplegado se regenera al arrancar: no hace falta copiarlo.
    // Los logs quedan fuera para no pisar el archivo que svlogd está escribiendo.
    let exclude_war = format!("--exclude={name}/home/war");
    let exclude_logs = format!("--exclude={name}/logs");
    let result = util::run_tolerant(
        "tar",
        &[
            "czf",
            &out_s,
            "-C",
            &parent_s,
            &exclude_war,
            &exclude_logs,
            &name,
        ],
        &[1],
    );

    if was_running {
        start_and_wait(cfg)?;
    }
    result?;
    let size = fs::metadata(&out)
        .map(|meta| util::human_size(meta.len()))
        .unwrap_or_default();
    println!("Copia creada: {} ({size})", out.display());
    Ok(())
}

pub fn restore(cfg: &Config, file: PathBuf) -> Result<()> {
    let file = util::expand_home(&file);
    if !file.exists() {
        bail!("no existe {}", file.display());
    }
    let p = cfg.paths();
    let parent = p.root.parent().context("ruta de instalación inválida")?;

    let was_running = service::status().running;
    if was_running {
        service::stop()?;
        service::wait_stopped(Duration::from_secs(30));
        sleep(Duration::from_millis(500));
    }
    println!("Restaurando {} en {}", file.display(), parent.display());
    let file_s = file.to_string_lossy().to_string();
    let parent_s = parent.to_string_lossy().to_string();
    let result = util::run("tar", &["xzf", &file_s, "-C", &parent_s]);
    if was_running {
        start_and_wait(cfg)?;
    }
    result?;
    println!("Restauración completada. Comprueba con: termux-cicd status");
    Ok(())
}

pub fn uninstall(cfg: &Config, purge: bool) -> Result<()> {
    let p = cfg.paths();
    println!("Deteniendo Jenkins...");
    let _ = service::stop();
    if !service::wait_stopped(Duration::from_secs(30)) {
        println!("  Aviso: Jenkins sigue deteniéndose; puede quedar algún archivo bloqueado.");
    }
    sleep(Duration::from_millis(500));

    let boot = service::boot_script();
    if boot.exists() {
        fs::remove_file(&boot)?;
        println!("Eliminado {}", boot.display());
    }

    let dir = service::service_dir();
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
        println!("Eliminado {}", dir.display());
    }

    if purge {
        if p.root.exists() {
            fs::remove_dir_all(&p.root)?;
            println!("Eliminado {}", p.root.display());
        }
        let conf = config::config_path();
        if conf.exists() {
            fs::remove_file(&conf)?;
            println!("Eliminado {}", conf.display());
        }
    } else {
        println!(
            "Datos conservados en {} (usa --purge para borrarlos).",
            p.root.display()
        );
    }
    println!("Desinstalación completada.");
    Ok(())
}

pub fn config(cfg: &mut Config, action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            println!("Configuración: {}", config::config_path().display());
            print!("{}", toml::to_string_pretty(cfg)?);
            println!("\nContraseña: {}", cfg.paths().password().display());
            println!("Log:        {}", cfg.paths().log_file().display());
            Ok(())
        }
        ConfigAction::Set { key, value } => {
            match key.as_str() {
                "port" => cfg.port = value.parse().context("puerto inválido")?,
                "heap_mb" => cfg.heap_mb = value.parse().context("valor inválido")?,
                "metaspace_mb" => cfg.metaspace_mb = value.parse().context("valor inválido")?,
                "executors" => cfg.executors = value.parse().context("valor inválido")?,
                "url" => cfg.url = value,
                other => bail!(
                    "clave desconocida: {other} (usa port, heap_mb, metaspace_mb, executors o url)"
                ),
            }

            if cfg.port == 0 {
                bail!("el puerto no puede ser 0");
            }
            if cfg.heap_mb < 256 {
                bail!("heap_mb debe ser >= 256");
            }
            if cfg.metaspace_mb < 128 {
                bail!("metaspace_mb debe ser >= 128");
            }
            if cfg.executors == 0 || cfg.executors > 8 {
                bail!("executors debe estar entre 1 y 8");
            }
            cfg.save()?;

            if cfg.paths().war().exists() {
                jenkins::write_init_script(cfg)?;
                jenkins::write_url(cfg)?;
                service::write_files(cfg)?;
                if service::status().running {
                    service::restart()?;
                    println!("Configuración aplicada y Jenkins reiniciado.");
                } else {
                    println!("Configuración aplicada.");
                }
            } else {
                println!(
                    "Configuración guardada. Se aplicará al ejecutar `termux-cicd install`."
                );
            }
            Ok(())
        }
    }
}

pub fn github(cfg: &Config, action: GithubAction) -> Result<()> {
    match action {
        GithubAction::List => github_list(cfg),
        GithubAction::Add {
            repo,
            name,
            credentials_id,
            user,
            token,
            token_file,
            poll_minutes,
            no_scan,
        } => github_add(
            cfg,
            &repo,
            name,
            credentials_id,
            &user,
            token,
            token_file,
            poll_minutes,
            !no_scan,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn github_add(
    cfg: &Config,
    repo: &str,
    name: Option<String>,
    credentials_id: Option<String>,
    user: &str,
    token: Option<String>,
    token_file: Option<PathBuf>,
    poll_minutes: u32,
    scan: bool,
) -> Result<()> {
    ensure_installed(cfg)?;
    if !service::status().running {
        bail!("Jenkins no está en ejecución. Arranca con: termux-cicd start");
    }

    let (owner, repo_name) = parse_github_repo(repo)?;
    let job_name = name.unwrap_or_else(|| repo_name.clone());
    if job_name.trim().is_empty() {
        bail!("el nombre del job no puede estar vacío");
    }

    let token = resolve_token(token, token_file)?;
    let credential_id = match (credentials_id, token.is_some()) {
        (Some(id), _) => id,
        (None, true) => "github-pat".to_string(),
        (None, false) => String::new(),
    };

    let trigger = if poll_minutes == 0 {
        String::new()
    } else if (1..=60).contains(&poll_minutes) {
        format!("{poll_minutes}m")
    } else {
        bail!("poll_minutes debe estar entre 0 y 60");
    };

    println!("Configurando GitHub en Jenkins");
    println!("  Repositorio: {owner}/{repo_name}");
    println!("  Job:         {job_name}");
    if credential_id.is_empty() {
        println!("  Credencial:  ninguna (solo funciona con repositorios públicos)");
    } else if token.is_some() {
        println!("  Credencial:  {credential_id} (token nuevo o actualizado)");
    } else {
        println!("  Credencial:  {credential_id} (existente)");
    }
    println!(
        "  Escaneo:     {}",
        if trigger.is_empty() {
            "desactivado".to_string()
        } else {
            format!("cada {poll_minutes} min")
        }
    );

    let script = GITHUB_JOB_TEMPLATE
        .replace("{{JOB_NAME}}", &jenkins::groovy_escape(&job_name))
        .replace("{{OWNER}}", &jenkins::groovy_escape(&owner))
        .replace("{{REPO}}", &jenkins::groovy_escape(&repo_name))
        .replace("{{CREDENTIALS_ID}}", &jenkins::groovy_escape(&credential_id))
        .replace("{{USER}}", &jenkins::groovy_escape(user))
        .replace("{{TOKEN}}", &jenkins::groovy_escape(token.as_deref().unwrap_or("")))
        .replace("{{TRIGGER}}", &jenkins::groovy_escape(&trigger))
        .replace("{{SCAN}}", if scan { "true" } else { "false" });

    println!("  Aplicando configuración...");
    jenkins::run_groovy(cfg, &script)?;

    println!("\nJob listo: {job_name}");
    if scan {
        println!("  Escaneo inicial solicitado; aparecerá en unos segundos.");
        println!("  Consulta el resultado con: termux-cicd github list");
    }
    println!("  Requiere un Jenkinsfile en el repositorio.");
    Ok(())
}

fn github_list(cfg: &Config) -> Result<()> {
    ensure_installed(cfg)?;
    if !service::status().running {
        bail!("Jenkins no está en ejecución. Arranca con: termux-cicd start");
    }
    jenkins::run_groovy(cfg, GITHUB_LIST_TEMPLATE)
}

/// Acepta `https://github.com/owner/repo`, `git@github.com:owner/repo.git`
/// y la forma corta `owner/repo`.
pub fn parse_github_repo(input: &str) -> Result<(String, String)> {
    let mut text = input.trim();
    if text.is_empty() {
        bail!("no has indicado ningún repositorio");
    }

    if let Some(rest) = text.strip_prefix("git@github.com:") {
        text = rest;
    } else if let Some(rest) = text
        .strip_prefix("https://github.com/")
        .or_else(|| text.strip_prefix("http://github.com/"))
    {
        text = rest;
    } else if text.starts_with("https://") || text.starts_with("http://") || text.starts_with("git@")
    {
        bail!("solo se admiten repositorios de github.com: {input}");
    }

    let text = text.trim_end_matches('/');
    let text = text.strip_suffix(".git").unwrap_or(text);
    let mut parts = text.split('/');
    let owner = parts.next().unwrap_or("").trim();
    let repo = parts.next().unwrap_or("").trim();
    if parts.next().is_some() {
        bail!("formato de repositorio no válido: {input} (usa owner/repo)");
    }

    let valid = |value: &str| {
        !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if !valid(owner) || !valid(repo) {
        bail!("formato de repositorio no válido: {input} (usa owner/repo)");
    }
    Ok((owner.to_string(), repo.to_string()))
}

fn resolve_token(token: Option<String>, token_file: Option<PathBuf>) -> Result<Option<String>> {
    if let Some(path) = token_file {
        let path = util::expand_home(&path);
        let value = fs::read_to_string(&path)
            .with_context(|| format!("no se pudo leer el token de {}", path.display()))?;
        let value = value.trim().to_string();
        if value.is_empty() {
            bail!("el archivo {} está vacío", path.display());
        }
        return Ok(Some(value));
    }
    if let Some(value) = token {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Ok(Some(value));
        }
    }
    if let Ok(value) = std::env::var("GITHUB_TOKEN") {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn ensure_installed(cfg: &Config) -> Result<()> {
    if !cfg.paths().war().exists() {
        bail!(
            "Jenkins no está instalado en {}. Ejecuta: termux-cicd install",
            cfg.paths().root.display()
        );
    }
    Ok(())
}

fn start_and_wait(cfg: &Config) -> Result<()> {
    service::start()?;
    let ready = jenkins::wait_http(cfg.port, Duration::from_secs(90))?;
    if !ready {
        println!("  Aviso: Jenkins sigue arrancando; revisa `termux-cicd status`.");
    }
    Ok(())
}

fn display_url(cfg: &Config) -> String {
    if cfg.url.trim().is_empty() {
        format!(
            "http://{}:{}/",
            termux::lan_ip().unwrap_or_else(|| "127.0.0.1".into()),
            cfg.port
        )
    } else {
        cfg.url.trim().to_string()
    }
}

fn step(number: u8, total: u8, text: &str) {
    println!("[{number}/{total}] {text}");
}

fn print_summary(cfg: &Config, url: &str, password: &str, ready: bool) {
    let p = cfg.paths();
    println!("Jenkins {}", if ready { "listo" } else { "instalado" });
    println!("  URL:      {url}");
    println!("  Usuario:  admin");
    println!("  Password: {password}");
    println!("  Guardada: {}", p.password().display());
    println!();
    println!("Comandos útiles:");
    println!("  termux-cicd status");
    println!("  termux-cicd logs -f");
    println!("  termux-cicd doctor");
    println!();
    println!(
        "Acceso remoto: instala la app Tailscale y abre la IP 100.x.y.z en el puerto {}.",
        cfg.port
    );
    println!(
        "Arranque al encender: instala Termux:Boot y quita la optimización de batería a Termux."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acepta_url_https() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo").unwrap(),
            ("owner".into(), "repo".into())
        );
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git").unwrap(),
            ("owner".into(), "repo".into())
        );
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo/").unwrap(),
            ("owner".into(), "repo".into())
        );
    }

    #[test]
    fn acepta_ssh_y_forma_corta() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo.git").unwrap(),
            ("owner".into(), "repo".into())
        );
        assert_eq!(
            parse_github_repo("owner/repo").unwrap(),
            ("owner".into(), "repo".into())
        );
        assert_eq!(
            parse_github_repo("  owner/repo  ").unwrap(),
            ("owner".into(), "repo".into())
        );
    }

    #[test]
    fn rechaza_formatos_invalidos() {
        assert!(parse_github_repo("").is_err());
        assert!(parse_github_repo("repo").is_err());
        assert!(parse_github_repo("https://gitlab.com/owner/repo").is_err());
        assert!(parse_github_repo("owner/repo/extra").is_err());
        assert!(parse_github_repo("owner/re po").is_err());
    }
}
