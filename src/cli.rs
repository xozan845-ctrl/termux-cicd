use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "termux-cicd",
    version,
    about = "Convierte Termux en un servidor CI/CD con Jenkins",
    long_about = "Automatiza la instalación, el arranque y el mantenimiento de Jenkins como servidor CI/CD personal en Termux (Android)."
)]
pub struct Cli {
    /// Directorio de instalación (por defecto ~/jenkins)
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Puerto HTTP de Jenkins
    #[arg(long, global = true, value_name = "PUERTO")]
    pub port: Option<u16>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Instala o repara Jenkins (idempotente)
    Install {
        /// Versión exacta de Jenkins (por defecto, la última LTS)
        #[arg(long)]
        version: Option<String>,
        /// Reinstala los plugins aunque ya existan
        #[arg(long)]
        force_plugins: bool,
    },
    /// Arranca el servicio
    Start,
    /// Detiene el servicio
    Stop,
    /// Reinicia el servicio
    Restart,
    /// Muestra el estado y la URL de acceso
    Status,
    /// Muestra el log del servicio
    Logs {
        /// Sigue el log en tiempo real
        #[arg(short, long)]
        follow: bool,
        /// Número de líneas a mostrar
        #[arg(short = 'n', long, default_value_t = 200)]
        lines: usize,
    },
    /// Comprueba requisitos y estado del servidor
    Doctor,
    /// Actualiza Jenkins a la última LTS
    Update {
        /// Versión exacta a instalar
        #[arg(long)]
        version: Option<String>,
    },
    /// Crea una copia de seguridad
    Backup {
        /// Archivo de salida (por defecto ~/jenkins-backup-FECHA.tgz)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Restaura una copia de seguridad
    Restore {
        /// Archivo .tgz creado con `backup`
        file: PathBuf,
    },
    /// Desinstala el servicio (conserva los datos salvo --purge)
    Uninstall {
        /// Borra también JENKINS_HOME, plugins y credenciales
        #[arg(long)]
        purge: bool,
    },
    /// Muestra o cambia la configuración
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Muestra la configuración actual
    Show,
    /// Cambia un valor: port, heap_mb, metaspace_mb, executors, url
    Set { key: String, value: String },
}
