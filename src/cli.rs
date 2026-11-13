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
    /// Configura repositorios de GitHub en Jenkins
    Github {
        #[command(subcommand)]
        action: GithubAction,
    },
    /// Instala Jenkins (si hace falta) y conecta el proyecto actual a GitHub
    Setup {
        /// URL del repositorio o formato owner/repo (por defecto, el remoto origin)
        repo: Option<String>,
        /// Directorio del proyecto (por defecto, el actual)
        #[arg(long, value_name = "DIR", default_value = ".")]
        dir: PathBuf,
        /// Nombre del job (por defecto, el nombre del repositorio)
        #[arg(long)]
        name: Option<String>,
        /// Token de GitHub (mejor usa GITHUB_TOKEN o --token-file)
        #[arg(long, env = "GITHUB_TOKEN")]
        token: Option<String>,
        /// Archivo que contiene el token
        #[arg(long, value_name = "ARCHIVO")]
        token_file: Option<PathBuf>,
        /// Minutos entre escaneos (0 = desactivado)
        #[arg(long, default_value_t = 5)]
        poll_minutes: u32,
        /// Hacer commit y push del Jenkinsfile generado
        #[arg(long)]
        push: bool,
    },
    /// Muestra o cambia la configuración
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum GithubAction {
    /// Crea o actualiza credenciales y un Multibranch Pipeline
    Add {
        /// URL del repositorio o formato owner/repo (por defecto, el remoto origin)
        repo: Option<String>,
        /// Directorio del proyecto (por defecto, el actual)
        #[arg(long, value_name = "DIR", default_value = ".")]
        dir: PathBuf,
        /// Nombre del job (por defecto, el nombre del repositorio)
        #[arg(long)]
        name: Option<String>,
        /// ID de una credencial existente (por defecto github-pat si hay token)
        #[arg(long, value_name = "ID")]
        credentials_id: Option<String>,
        /// Usuario de GitHub para la credencial
        #[arg(long, default_value = "x-access-token")]
        user: String,
        /// Token de GitHub (mejor usa GITHUB_TOKEN o --token-file)
        #[arg(long, env = "GITHUB_TOKEN")]
        token: Option<String>,
        /// Archivo que contiene el token
        #[arg(long, value_name = "ARCHIVO")]
        token_file: Option<PathBuf>,
        /// Minutos entre escaneos (0 = desactivado)
        #[arg(long, default_value_t = 5)]
        poll_minutes: u32,
        /// No lanzar el escaneo inicial
        #[arg(long)]
        no_scan: bool,
        /// No generar un Jenkinsfile si el proyecto no tiene uno
        #[arg(long)]
        no_jenkinsfile: bool,
        /// Hacer commit y push del Jenkinsfile generado
        #[arg(long)]
        push: bool,
    },
    /// Lista los Multibranch Pipeline configurados
    List,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Muestra la configuración actual
    Show,
    /// Cambia un valor: port, heap_mb, metaspace_mb, executors, url
    Set { key: String, value: String },
}
