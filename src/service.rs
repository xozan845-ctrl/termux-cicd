use crate::config::{Config, Paths};
use crate::util;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::{Duration, Instant};

const RUN_TEMPLATE: &str = include_str!("../templates/jenkins-run.sh");
const LOG_TEMPLATE: &str = include_str!("../templates/jenkins-log-run.sh");
const BOOT_TEMPLATE: &str = include_str!("../templates/boot-jenkins.sh");

pub fn service_dir() -> PathBuf {
    util::prefix().join("var/service/jenkins")
}

pub fn boot_script() -> PathBuf {
    util::home().join(".termux/boot/10-jenkins.sh")
}

pub fn write_files(cfg: &Config) -> Result<()> {
    let p = cfg.paths();
    let dir = service_dir();
    fs::create_dir_all(dir.join("log"))?;
    fs::create_dir_all(p.logs())?;

    let home = util::home().to_string_lossy().to_string();
    let prefix = util::prefix().to_string_lossy().to_string();
    let jenkins_home = p.home().to_string_lossy().to_string();
    let war = p.war().to_string_lossy().to_string();
    let log_dir = p.logs().to_string_lossy().to_string();

    let run = RUN_TEMPLATE
        .replace("{{HOME}}", &home)
        .replace("{{PREFIX}}", &prefix)
        .replace("{{JENKINS_HOME}}", &jenkins_home)
        .replace("{{WAR}}", &war)
        .replace("{{PORT}}", &cfg.port.to_string())
        .replace("{{HEAP_MB}}", &cfg.heap_mb.to_string())
        .replace("{{META_MB}}", &cfg.metaspace_mb.to_string());
    fs::write(dir.join("run"), run)?;
    util::set_executable(&dir.join("run"))?;

    let log = LOG_TEMPLATE.replace("{{LOG_DIR}}", &log_dir);
    fs::write(dir.join("log/run"), log)?;
    util::set_executable(&dir.join("log/run"))?;

    let boot = BOOT_TEMPLATE
        .replace("{{HOME}}", &home)
        .replace("{{PREFIX}}", &prefix);
    if let Some(parent) = boot_script().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(boot_script(), boot)?;
    util::set_executable(&boot_script())?;

    Ok(())
}

fn sv(args: &[&str]) -> Result<Output> {
    let prefix = util::prefix();
    Command::new(prefix.join("bin/sv"))
        .args(args)
        .env("SVDIR", prefix.join("var/service"))
        .env("LOGDIR", prefix.join("var/log"))
        .output()
        .context("no se pudo ejecutar `sv` (¿está instalado termux-services?)")
}

pub fn ensure_daemon() -> Result<()> {
    let daemon = util::prefix().join("bin/service-daemon");
    if daemon.exists() {
        let _ = Command::new(daemon).arg("start").output();
        sleep(Duration::from_millis(1200));
    }
    Ok(())
}

pub fn start() -> Result<()> {
    ensure_daemon()?;
    for _ in 0..10 {
        let _ = fs::remove_file(service_dir().join("down"));
        let out = sv(&["up", "jenkins"])?;
        if out.status.success() {
            return Ok(());
        }
        sleep(Duration::from_millis(500));
    }
    bail!("no se pudo arrancar el servicio jenkins")
}

pub fn stop() -> Result<()> {
    let _ = sv(&["down", "jenkins"]);
    Ok(())
}

pub fn restart() -> Result<()> {
    let _ = stop();
    sleep(Duration::from_millis(700));
    start()
}

/// Espera a que el servicio deje de estar en ejecución.
pub fn wait_stopped(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !status().running {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(500));
    }
}

#[derive(Debug, Clone)]
pub struct Status {
    pub running: bool,
    pub pid: Option<u32>,
    pub raw: String,
}

pub fn status() -> Status {
    match sv(&["status", "jenkins"]) {
        Ok(out) => {
            let raw = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
            .trim()
            .to_string();
            let running = raw.contains("run: jenkins:");
            let pid = if running {
                let main = raw.split(';').next().unwrap_or("");
                main.find("(pid ").and_then(|i| {
                    let rest = &main[i + 5..];
                    rest.split(|c: char| !c.is_ascii_digit())
                        .next()
                        .and_then(|digits| digits.parse::<u32>().ok())
                })
            } else {
                None
            };
            Status { running, pid, raw }
        }
        Err(err) => Status {
            running: false,
            pid: None,
            raw: err.to_string(),
        },
    }
}

pub fn tail(p: &Paths, lines: usize, follow: bool) -> Result<()> {
    let file = p.log_file();
    if !file.exists() {
        bail!("no existe el log {}", file.display());
    }
    let n = lines.to_string();
    let path = file.to_string_lossy().to_string();
    if follow {
        util::run("tail", &["-n", &n, "-f", &path])
    } else {
        util::run("tail", &["-n", &n, &path])
    }
}
