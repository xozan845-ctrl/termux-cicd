mod cli;
mod commands;
mod config;
mod doctor;
mod jenkins;
mod service;
mod termux;
mod util;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    // Comportarse como un CLI Unix normal al cerrar la tubería (ej. `| head`).
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(err) = run() {
        eprintln!("\nError: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut cfg = config::Config::load()?;

    if let Some(root) = cli.root {
        let root = util::expand_home(&root);
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()?.join(root)
        };
        cfg.root = root;
    }
    if let Some(port) = cli.port {
        cfg.port = port;
    }

    match cli.command {
        Command::Install {
            version,
            force_plugins,
        } => commands::install(&mut cfg, version, force_plugins),
        Command::Start => commands::start(&cfg),
        Command::Stop => commands::stop(),
        Command::Restart => commands::restart(&cfg),
        Command::Status => commands::status(&cfg),
        Command::Logs { follow, lines } => commands::logs(&cfg, follow, lines),
        Command::Doctor => doctor::run(&cfg),
        Command::Update { version } => commands::update(&mut cfg, version),
        Command::Backup { output } => commands::backup(&cfg, output),
        Command::Restore { file } => commands::restore(&cfg, file),
        Command::Uninstall { purge } => commands::uninstall(&cfg, purge),
        Command::Config { action } => commands::config(&mut cfg, action),
    }
}
