use crate::config::Config;
use crate::jenkins;
use crate::service;
use crate::termux;
use crate::util;
use anyhow::{bail, Result};

pub fn run(cfg: &Config) -> Result<()> {
    let p = cfg.paths();
    let mut failures = 0u32;
    let mut total = 0u32;

    println!("Diagnóstico de termux-cicd\n");

    check(
        "Entorno Termux",
        util::prefix().starts_with("/data/data/com.termux"),
        &format!("PREFIX={}", util::prefix().display()),
        &mut total,
        &mut failures,
    );
    check(
        "Arquitectura ARM64",
        termux::is_arm64(),
        &termux::arch(),
        &mut total,
        &mut failures,
    );
    check(
        "Java instalado",
        util::which("java").is_some(),
        &termux::java_version().unwrap_or_else(|| "no encontrado".into()),
        &mut total,
        &mut failures,
    );
    check(
        "Git instalado",
        util::which("git").is_some(),
        util::quiet("git", &["--version"])
            .unwrap_or_default()
            .trim(),
        &mut total,
        &mut failures,
    );
    check(
        "Curl instalado",
        util::which("curl").is_some(),
        "",
        &mut total,
        &mut failures,
    );
    let fontconfig = util::prefix().join("etc/fonts/fonts.conf");
    check(
        "Fontconfig configurado",
        fontconfig.exists(),
        &fontconfig.display().to_string(),
        &mut total,
        &mut failures,
    );
    check(
        "Jenkins instalado",
        p.war().exists() && p.home().exists(),
        &p.root.display().to_string(),
        &mut total,
        &mut failures,
    );

    let st = service::status();
    check(
        "Servicio en ejecución",
        st.running,
        st.raw.split(';').next().unwrap_or("").trim(),
        &mut total,
        &mut failures,
    );

    let code = jenkins::http_code(cfg.port, "/login").unwrap_or_else(|| "sin respuesta".into());
    check(
        &format!("HTTP :{} responde", cfg.port),
        code == "200",
        &format!("código {code}"),
        &mut total,
        &mut failures,
    );

    if let Some(free) = termux::free_disk_mb(&p.root) {
        check(
            "Disco libre > 2 GB",
            free > 2048,
            &format!("{free} MB"),
            &mut total,
            &mut failures,
        );
    }
    if let Some(mem) = termux::mem_available_mb() {
        check(
            "Memoria disponible > 500 MB",
            mem > 500,
            &format!("{mem} MB"),
            &mut total,
            &mut failures,
        );
    }

    println!();
    warn(
        "Termux:Boot instalado",
        termux::app_installed("com.termux.boot"),
        "necesario para arrancar Jenkins al encender el móvil",
    );
    println!("  ℹ Batería: revisa Ajustes de Android → Batería → Termux → Sin restricciones");

    println!();
    if failures == 0 {
        println!("Correcto: {total} comprobaciones superadas.");
        Ok(())
    } else {
        bail!("{failures} de {total} comprobaciones fallaron");
    }
}

fn check(name: &str, ok: bool, detail: &str, total: &mut u32, failures: &mut u32) {
    *total += 1;
    if ok {
        println!("  ✔ {name}");
    } else {
        *failures += 1;
        if detail.is_empty() {
            println!("  ✖ {name}");
        } else {
            println!("  ✖ {name} ({detail})");
        }
    }
}

fn warn(name: &str, ok: bool, hint: &str) {
    if ok {
        println!("  ✔ {name}");
    } else {
        println!("  ⚠ {name}: {hint}");
    }
}
