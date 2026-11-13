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
- Configura repositorios de GitHub: credencial, Multibranch Pipeline y polling.
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

### La vía corta: un solo comando

Dentro de tu proyecto (con `git remote origin` apuntando a GitHub):

```sh
termux-cicd setup
```

Eso instala Jenkins si aún no existe, crea la credencial (si encuentra un
token), configura el Multibranch Pipeline, programa el polling y genera un
`Jenkinsfile` para tu stack si falta. No hay que pasar ni el repo ni el token.

```sh
termux-cicd setup --push    # además hace commit y push del Jenkinsfile
```

### Comandos completos

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

# GitHub
termux-cicd github add usuario/repo
termux-cicd github list
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

### Automático (recomendado)

Lo normal es no tener que escribir nada:

```sh
cd mi-proyecto
termux-cicd setup          # instala + credencial + job + Jenkinsfile
termux-cicd github add     # solo la parte de GitHub
```

`repo` es opcional: se deduce de `git remote get-url origin`. También puedes
pasarlo explícitamente:

```sh
termux-cicd github add https://github.com/usuario/repo
termux-cicd github add usuario/repo --name mi-proyecto --poll-minutes 10
termux-cicd github add usuario/repo --credentials-id mi-pat-existente --no-scan
termux-cicd github list
```

Formatos de repositorio admitidos: `https://github.com/usuario/repo`,
`git@github.com:usuario/repo.git` y `usuario/repo`.

**Token automático.** Si no pasas `--token`, el CLI busca en este orden:

1. `GITHUB_TOKEN`
2. `GH_TOKEN`
3. `~/.config/termux-cicd/github-token`
4. `~/.config/github-token`
5. `~/.github-token`
6. `~/.config/gh/hosts.yml` (si usas GitHub CLI)
7. `gh auth token`

Si no encuentra ninguno, configura el job para acceso anónimo: válido para
repositorios públicos. Para privados, exporta `GITHUB_TOKEN` una vez.

**Jenkinsfile automático.** Si el proyecto local no tiene `Jenkinsfile`, se
genera uno según el stack detectado:

| Archivo detectado | Stack | Pipeline |
|---|---|---|
| `Cargo.toml` | Rust | `cargo build --release` + `cargo test` |
| `package.json` | Node.js | `npm ci`/`install` + `test` + `build` |
| `pyproject.toml`, `requirements.txt`, `setup.py` | Python | venv + pytest/unittest |
| `pom.xml` | Maven | `./mvnw` o `mvn verify` |
| `build.gradle(.kts)` | Gradle | `./gradlew` o `gradle build` |
| `go.mod` | Go | `go test` + `go build` |
| ninguno | genérico | inspección del repo |

Usa `--no-jenkinsfile` para no generarlo y `--push` para hacer commit y push
del archivo generado automáticamente.

El comando:

1. Deduce `owner/repo` del remoto `origin` si no lo indicas.
2. Crea (o actualiza) la credencial `github-pat` de tipo *Username with
   password* con el token como contraseña.
3. Crea un **Multibranch Pipeline** con GitHub Branch Source.
4. Descubre todas las ramas y PRs.
5. Programa el escaneo periódico (`5m` por defecto, `0` lo desactiva).
6. Genera el `Jenkinsfile` si falta.
7. Lanza el escaneo inicial salvo que uses `--no-scan`.

Necesita un `Jenkinsfile` en la rama. Si no existe, el escaneo termina bien
pero no crea ninguna rama compilable (así lo indica el log de indexación).

### Manual

También puedes hacerlo desde la interfaz:

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

## Seguridad del token

- `--token` funciona, pero queda visible en el historial del shell y en `ps`.
  Prefiere `GITHUB_TOKEN` o `--token-file`.
- El token se envía a Jenkins por la entrada estándar de su CLI, nunca como
  argumento de proceso.
- La contraseña de Jenkins se pasa en un archivo temporal `0600` que se borra
  al terminar.
- Usa un PAT con el mínimo permiso necesario (`repo` para repos privados,
  `public_repo` para públicos).

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

- [x] Comando `github` para crear credenciales y el Multibranch Pipeline vía API.
- [ ] Perfiles de plugins por stack (Node, Python, Rust, Go...).
- [ ] `doctor --fix`.
- [ ] Instalación de toolchains de build (`nodejs`, `python`, `golang`).
- [ ] Tests de integración en un Termux limpio.

## Licencia

MIT.
