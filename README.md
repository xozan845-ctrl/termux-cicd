# termux-cicd

CLI en Rust que convierte un móvil Android con **Termux** en un servidor
**CI/CD personal con Jenkins**, de forma automatizada e idempotente.

Nace de montar Jenkins a mano en un móvil: instalar Java, descargar el WAR,
resolver las fuentes de Android, preparar el servicio `runit`, generar
credenciales y dejarlo arrancando solo al encender. Todo eso ahora es un comando.

## Qué hace

- Detecta Termux y la arquitectura.
- Instala `openjdk-21`, `git`, `fontconfig`, `termux-services`, etc.
- Descarga la última **Jenkins LTS** (o la versión que le pidas).
- Instala los plugins esenciales de Git/GitHub/Pipeline con
  `jenkins-plugin-manager`.
- Genera un usuario `admin` con contraseña aleatoria.
- Crea el servicio `runit` (`sv`) y el script de arranque para **Termux:Boot**.
- Arranca Jenkins, espera a que autentique y muestra la URL.
- Gestiona el ciclo de vida: estado, logs, actualización, backup/restore y
  desinstalación.

## Requisitos

- Android + Termux (probado en aarch64, Android 16).
- ~1 GB de disco para Jenkins, plugins y Java.
- Termux:Boot (opcional) para arranque automático.
- App de Tailscale (opcional) para acceso remoto sin abrir puertos.

## Compilar e instalar

```sh
pkg install rust
git clone <tu-repo> termux-cicd   # o copia el directorio
cd termux-cicd
cargo build --release
install -m755 target/release/termux-cicd "$PREFIX/bin/termux-cicd"
```

## Uso

```sh
# Instalación completa (idempotente; se puede repetir para reparar)
termux-cicd install

# Con una versión concreta y forzando plugins
termux-cicd install --version 2.580.2 --force-plugins

# Ciclo de vida
termux-cicd status
termux-cicd start
termux-cicd stop
termux-cicd restart
termux-cicd logs -f
termux-cicd logs -n 50

# Diagnóstico
termux-cicd doctor

# Mantenimiento
termux-cicd update
termux-cicd backup
termux-cicd backup --output ~/backups/jenkins.tgz
termux-cicd restore ~/backups/jenkins.tgz
termux-cicd uninstall
termux-cicd uninstall --purge

# Configuración
termux-cicd config show
termux-cicd config set port 8081
termux-cicd config set heap_mb 1500
termux-cicd config set executors 2
termux-cicd config set url http://mi-movil.tailnet.ts.net:8080/
```

Opciones globales: `--root DIR` (por defecto `~/jenkins`) y `--port PUERTO`.

## Estructura en disco

| Ruta | Contenido |
|---|---|
| `~/jenkins/jenkins.war` | Jenkins LTS |
| `~/jenkins/home` | `JENKINS_HOME`: jobs, plugins, credenciales |
| `~/jenkins/home/init.groovy.d` | Script de configuración inicial |
| `~/jenkins/logs/current` | Log del servicio |
| `~/.config/termux-cicd/config.toml` | Configuración del CLI |
| `$PREFIX/var/service/jenkins/` | Servicio runit |
| `~/.termux/boot/10-jenkins.sh` | Arranque con Termux:Boot |

La contraseña de `admin` se guarda en `~/jenkins/home/admin-password.txt`
(permisos `0600`). No la subas a ningún repo.

## Cómo funciona el servicio

`termux-services` usa `runit`. El CLI escribe:

- `run`: arranca Java con `JENKINS_HOME`, `FONTCONFIG_PATH` y el puerto.
- `log/run`: `svlogd` para el log.
- `~/.termux/boot/10-jenkins.sh`: `termux-wake-lock` + `service-daemon` +
  `sv-enable jenkins`.

Si Jenkins se cae, `runit` lo reinicia.

## Conectar un proyecto de GitHub

1. En Jenkins: **New Item → Multibranch Pipeline**.
2. Branch Source → GitHub → tu usuario/repo.
3. Si es privado, crea una credencial *Username with password* usando tu
   usuario y un **Personal Access Token** como contraseña.
4. En *Scan Multibranch Pipeline Triggers* activa
   *Periodically if not otherwise run* → `5 minutes`.
5. Añade un `Jenkinsfile` al repo.

Sin IP pública no hay webhooks: el **polling** cada 5 minutos es la opción
estable. Para builds instantáneos haría falta exponer Jenkins (Cloudflare
Tunnel, ngrok...), que no es el objetivo de esta versión.

## Acceso remoto

La vía recomendada en Android es la **app oficial de Tailscale** (no los
binarios de Termux, que no corren sobre bionic). Actívala y abre
`http://<IP-tailnet>:8080` desde otro dispositivo de tu tailnet.

## Notas

- Java en Android necesita `fontconfig` y una fuente; el CLI lo instala y
  configura. Sin eso Jenkins falla con `Fontconfig head is null`.
- El servicio usa `-Xmx1200m` por defecto. Ajusta `heap_mb` según tu móvil.
- Jenkins escucha en `0.0.0.0`; úsalo solo en tu LAN/tailnet.
- `backup` detiene Jenkins durante la copia para que `JENKINS_HOME` sea
  consistente.
- `restore` sobrescribe el directorio de instalación actual.

## Roadmap

- [ ] Comando `github` para crear credenciales y el Multibranch Pipeline vía API.
- [ ] Perfiles de plugins por stack (Node, Python, Rust, Go...).
- [ ] `doctor --fix`.
- [ ] Instalación de toolchains de build (`nodejs`, `python`, `golang`).
- [ ] Tests de integración en un Termux limpio.

## Licencia

MIT.
