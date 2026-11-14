# termux-cicd

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Termux%20%2F%20Android-green.svg)]()

CLI en Rust que convierte un móvil Android con **Termux** en un servidor
**CI/CD personal con Jenkins**, de forma automatizada, idempotente y
reproducible.

Nace de montar Jenkins a mano en un móvil: instalar Java, descargar el WAR,
resolver las fuentes de Android, preparar el servicio `runit`, generar
credenciales y dejarlo arrancando solo al encender. Todo eso ahora es un
comando.

> Estado: MVP funcional, probado en Android 16 / aarch64 con Jenkins LTS
> 2.580.2. No requiere root.

---

## Índice

- [Qué problema resuelve](#qué-problema-resuelve)
- [Características](#características)
- [Arquitectura](#arquitectura)
- [Requisitos](#requisitos)
- [Instalación](#instalación)
- [Inicio rápido](#inicio-rápido)
- [Referencia de comandos](#referencia-de-comandos)
- [Cómo funciona por dentro](#cómo-funciona-por-dentro)
- [Configuración](#configuración)
- [Integración con GitHub](#integración-con-github)
- [Migración desde GitHub Actions](#migración-desde-github-actions)
- [Estructura en disco](#estructura-en-disco)
- [Seguridad](#seguridad)
- [Acceso remoto](#acceso-remoto)
- [Pruebas](#pruebas)
- [Solución de problemas](#solución-de-problemas)
- [Limitaciones y no objetivos](#limitaciones-y-no-objetivos)
- [Roadmap](#roadmap)
- [Contribuir](#contribuir)
- [Licencia](#licencia)

---

## Qué problema resuelve

Montar Jenkins en Termux a mano implica, como mínimo:

1. Instalar `openjdk-21`, `git`, `curl`, `fontconfig` y `termux-services`.
2. Descargar el WAR correcto de Jenkins LTS.
3. Instalar plugins con `jenkins-plugin-manager`.
4. Crear un usuario administrador y activar seguridad.
5. Configurar fuentes para que Java no falle con `Fontconfig head is null`.
6. Escribir un servicio `runit` que arranque Jenkins y lo reinicie si cae.
7. Crear un script de arranque para Termux:Boot.
8. Generar URL, credenciales y scripts de inicialización.
9. Configurar GitHub: credencial, Multibranch Pipeline y polling.

`termux-cicd` encapsula todo lo anterior en comandos idempotentes. Puedes
ejecutarlo varias veces sin miedo: reutiliza lo que ya existe y solo repara lo
que falta.

### Filosofía

- **Idempotente**: `install` se puede repetir; no duplica credenciales ni
  vuelve a descargar lo que ya está.
- **Sin root**: todo vive en `$HOME` y `$PREFIX`.
- **Sin preguntas**: deduce repositorio, token y stack cuando puede.
- **Reproducible**: la configuración queda en archivos versionables y el
  servicio en `runit`.
- **Seguro por defecto**: no expone Jenkins a Internet; usa LAN o Tailscale.

---

## Características

- Detección de Termux, arquitectura y herramientas del sistema.
- Instalación de paquetes con `pkg`.
- Descarga de la última **Jenkins LTS** o de una versión exacta.
- Instalación de 12 plugins esenciales de Git, GitHub y Pipeline.
- Usuario `admin` con contraseña aleatoria y permisos `0600`.
- Servicio `runit` con reinicio automático y log rotado con `svlogd`.
- Arranque automático al encender con **Termux:Boot**.
- `termux-wake-lock` para que Android no duerma el proceso.
- Ciclo de vida: `start`, `stop`, `restart`, `status`, `logs`.
- Diagnóstico con `doctor` (11 comprobaciones + avisos).
- Actualización conservando el WAR anterior.
- Backup/restore consistente de `JENKINS_HOME`.
- Configuración persistentes en TOML.
- Integración GitHub: credencial, Multibranch Pipeline, escaneo periódico y
  generación automática de `Jenkinsfile` según el stack.
- Conversión de workflows de `.github/workflows` a `Jenkinsfile` con
  `migrate`, conservando pasos, dependencias, variables y artefactos.
- Comando `setup` que hace todo en un paso dentro de un proyecto.

---

## Arquitectura

```
┌─────────────────────────────────────────────────────────────┐
│ Android + Termux                                            │
│                                                             │
│  termux-cicd (Rust)                                         │
│    ├── pkg: openjdk-21, git, curl, fontconfig, services      │
│    ├── ~/jenkins/                                           │
│    │     ├── jenkins.war        Jenkins LTS                 │
│    │     ├── home/              JENKINS_HOME                │
│    │     ├── logs/current       log del servicio            │
│    │     ├── url.txt            URL configurada             │
│    │     └── .version           versión instalada           │
│    ├── $PREFIX/var/service/jenkins/                         │
│    │     ├── run               java -jar jenkins.war        │
│    │     └── log/run           svlogd -tt                   │
│    └── ~/.termux/boot/10-jenkins.sh                         │
│                                                             │
│  runit ──> java ──> Jenkins :8080                           │
└─────────────────────────────────────────────────────────────┘
```

### Componentes

| Componente | Responsabilidad |
|---|---|
| `src/cli.rs` | Definición de comandos y opciones con `clap`. |
| `src/commands.rs` | Instalación, ciclo de vida, backup, GitHub y `setup`. |
| `src/migrate.rs` | Conversión de GitHub Actions a `Jenkinsfile`. |
| `src/jenkins.rs` | Versiones, WAR, plugins, init Groovy, CLI y readiness. |
| `src/service.rs` | Servicio `runit`, log y arranque con Termux:Boot. |
| `src/termux.rs` | Paquetes, IP LAN, memoria, disco y apps Android. |
| `src/config.rs` | Rutas y configuración TOML. |
| `src/doctor.rs` | Diagnóstico del entorno. |
| `src/util.rs` | Descargas, permisos, tamaños, red, `git`/`tar`/`curl`. |
| `templates/` | Scripts y Groovy embebidos con `include_str!`. |

El binario es autosuficiente: las plantillas se compilan dentro de él con
`include_str!`, así que no depende de archivos externos en tiempo de ejecución.

---

## Requisitos

- **Android** con **Termux** instalado desde F-Droid o GitHub (no la versión
  obsoleta de Play Store).
- Arquitectura **aarch64/arm64** (probada). Otras arquitecturas pueden
  funcionar, pero no están validadas.
- **Rust** para compilar: `pkg install rust` (probado con 1.96).
- **~1 GB libres** para Java, Jenkins, plugins y dependencias.
- **2 GB de RAM** recomendados. Con menos, baja `heap_mb`.
- Red local para el primer acceso.
- Opcional: **Termux:Boot** para arranque automático.
- Opcional: **app de Tailscale** para acceso remoto sin abrir puertos.

> Importante: Termux y Termux:Boot deben venir de la misma fuente
> (F-Droid/GitHub) para que compartan firma y el arranque funcione.

---

## Instalación

### Desde el código fuente

```sh
pkg update
pkg install rust git curl tar
git clone https://github.com/tu-usuario/termux-cicd.git
cd termux-cicd
cargo build --release
install -m755 target/release/termux-cicd "$PREFIX/bin/termux-cicd"
termux-cicd --version
```

Si actualizas el código:

```sh
cd termux-cicd
git pull
cargo build --release
install -m755 target/release/termux-cicd "$PREFIX/bin/termux-cicd"
```

### Instalar Jenkins

```sh
termux-cicd install
```

El comando ejecuta 8 pasos:

1. Comprueba que estás en Termux.
2. Instala `openjdk-21`, `git`, `curl`, `tar`, `fontconfig`,
   `fontconfig-utils` y `termux-services`.
3. Crea `~/jenkins/{home,plugins,init.groovy.d,logs}`.
4. Genera o reutiliza la contraseña de `admin`.
5. Descarga la última LTS (o la versión indicada).
6. Instala los plugins con `jenkins-plugin-manager`.
7. Escribe `init.groovy.d`, `url.txt`, el servicio `runit` y el script de
   Termux:Boot.
8. Arranca Jenkins y espera hasta 120 s a que autentique.

Al terminar muestra la URL, el usuario, la contraseña y dónde se guarda.

---

## Inicio rápido

### Caso 1: solo quiero Jenkins

```sh
termux-cicd install
termux-cicd status
termux-cicd doctor
```

### Caso 2: conectar un proyecto de GitHub

Dentro de tu proyecto, con `origin` apuntando a GitHub:

```sh
cd mi-proyecto
termux-cicd setup
```

`setup` instala Jenkins si hace falta, deduce el repo del remoto, busca el
token, configura el job y genera un `Jenkinsfile` para tu stack. Si hay
`.github/workflows`, intenta migrarlos primero.

### Caso 3: ya tengo GitHub Actions

```sh
cd mi-proyecto
termux-cicd migrate --dry-run   # ver el resultado sin escribir
termux-cicd migrate             # crear Jenkinsfile
termux-cicd setup               # conectar el repo a Jenkins
```

### Caso 4: solo configurar GitHub

```sh
cd mi-proyecto
termux-cicd github add
termux-cicd github list
```

### Caso 5: acceso remoto

1. Instala la app oficial de **Tailscale** en el móvil y en tu portátil.
2. Inicia sesión en la misma tailnet.
3. Abre `http://<IP-tailnet-del-móvil>:8080`.

---

## Referencia de comandos

### Opciones globales

| Opción | Descripción |
|---|---|
| `--root DIR` | Directorio de instalación. Por defecto `~/jenkins`. Acepta `~`. |
| `--port PUERTO` | Puerto HTTP de Jenkins. Por defecto `8080`. |
| `-h, --help` | Ayuda. |
| `-V, --version` | Versión del CLI. |

`--root` y `--port` afectan solo a la ejecución actual; para persistirlos usa
`config set` o ejecuta `install` (que guarda la configuración).

### `install`

```sh
termux-cicd install
termux-cicd install --version 2.580.2
termux-cicd install --force-plugins
```

- Descarga el WAR solo si cambia la versión.
- No reinstala plugins si ya están, salvo `--force-plugins`.
- Reutiliza la contraseña existente.
- Es seguro repetirlo para reparar una instalación.

### Ciclo de vida

```sh
termux-cicd start
termux-cicd stop
termux-cicd restart
termux-cicd status
```

- `start` arranca `runit` y espera hasta 90 s a que `/login` responda 200.
- `stop` baja el servicio; los datos no se tocan.
- `restart` para y arranca, esperando de nuevo.
- `status` muestra estado, PID, versión, URL, código HTTP, `JENKINS_HOME`,
  tamaño de datos y ruta de la contraseña.

### `logs`

```sh
termux-cicd logs
termux-cicd logs -n 50
termux-cicd logs -f
```

Lee `~/jenkins/logs/current`. `-n` indica líneas (200 por defecto); `-f`
sigue el archivo en tiempo real.

### `doctor`

```sh
termux-cicd doctor
```

Comprueba:

1. Entorno Termux.
2. Arquitectura ARM64.
3. Java instalado.
4. Git instalado.
5. Curl instalado.
6. Fontconfig configurado.
7. Jenkins instalado.
8. Servicio en ejecución.
9. HTTP `/login` responde 200.
10. Disco libre > 2 GB.
11. Memoria disponible > 500 MB.

Además avisa si falta **Termux:Boot** y recuerda revisar la optimización de
batería de Android. Devuelve código 1 si alguna comprobación crítica falla.

### `update`

```sh
termux-cicd update
termux-cicd update --version 2.580.3
```

1. Consulta la última LTS (o usa la indicada).
2. Si ya estás en esa versión, no hace nada.
3. Descarga el nuevo WAR como `jenkins.war.new`.
4. Detiene Jenkins.
5. Renombra el WAR actual a `jenkins.war.<versión>.bak`.
6. Instala el nuevo y actualiza `.version`.
7. Arranca y espera hasta 120 s.

El WAR anterior queda en `~/jenkins/` para volver atrás manualmente.

### `backup` y `restore`

```sh
termux-cicd backup
termux-cicd backup --output ~/backups/jenkins.tgz
termux-cicd restore ~/backups/jenkins.tgz
```

`backup`:

- Detiene Jenkins si está en marcha.
- Espera hasta 30 s a que el proceso muera de verdad.
- Crea un `.tgz` con todo el directorio de instalación **excepto**
  `home/war` y `logs` (se regeneran).
- Tolera el código 1 de `tar` cuando un archivo cambia mientras se lee.
- Rearranca Jenkins y espera a que responda.

`restore`:

- Detiene Jenkins si está en marcha.
- Extrae el `.tgz` en el directorio padre del `root` actual.
- Rearranca si estaba corriendo.

> El backup contiene `JENKINS_HOME`: jobs, credenciales, claves y la
> contraseña de `admin`. Trátalo como un secreto.
>
> El nombre del directorio dentro del tar es el `basename` del `root`. Si
> haces backup con `~/jenkins` y restauras con `--root ~/otro`, el tar
> restaurará `jenkins/`, no `otro/`. Para restore, mantén el mismo nombre de
> directorio.

### `uninstall`

```sh
termux-cicd uninstall
termux-cicd uninstall --purge
```

Sin `--purge`:

- Detiene Jenkins y espera a que termine.
- Elimina el script de Termux:Boot y el servicio `runit`.
- **Conserva** `~/jenkins` y la configuración.

Con `--purge`:

- Elimina también `~/jenkins` (jobs, plugins, credenciales) y
  `~/.config/termux-cicd/config.toml`.

### `config`

```sh
termux-cicd config show
termux-cicd config set port 8081
termux-cicd config set heap_mb 1500
termux-cicd config set metaspace_mb 384
termux-cicd config set executors 2
termux-cicd config set url http://mi-movil.tailnet.ts.net:8080/
```

| Clave | Rango | Efecto |
|---|---|---|
| `port` | 1–65535 | Reescribe el servicio y reinicia si está corriendo. |
| `heap_mb` | ≥ 256 | Memoria máxima de Java (`-Xmx`). |
| `metaspace_mb` | ≥ 128 | Metaspace de Java. |
| `executors` | 1–8 | Ejecutores concurrentes de Jenkins. |
| `url` | texto | URL pública usada por Jenkins y su CLI. |

Si Jenkins aún no está instalado, la configuración se guarda y se aplica en
el siguiente `install`.

### `github`

```sh
termux-cicd github add
termux-cicd github add usuario/repo
termux-cicd github add https://github.com/usuario/repo
termux-cicd github add git@github.com:usuario/repo.git
termux-cicd github list
```

Ver [Integración con GitHub](#integración-con-github) para el detalle.

### `setup`

```sh
termux-cicd setup
termux-cicd setup --push
termux-cicd setup usuario/repo --name mi-job --poll-minutes 10
```

Equivale a `install` (si hace falta) + `github add` con valores automáticos.
Es la vía recomendada para un proyecto nuevo.

### `migrate`

```sh
termux-cicd migrate
termux-cicd migrate --dry-run
termux-cicd migrate --output Jenkinsfile.ci --force
termux-cicd migrate --dir ~/proyecto
```

Convierte `.github/workflows/*.yml` y `*.yaml` en un `Jenkinsfile`
Declarative Pipeline. Si no hay workflows, falla y no escribe nada.

| Opción | Efecto |
|---|---|
| `--dir DIR` | Proyecto a convertir. Por defecto, el directorio actual. |
| `--output ARCHIVO` | Destino. Por defecto, `DIR/Jenkinsfile`. |
| `--force` | Sobrescribe el archivo de salida si ya existe. |
| `--dry-run` | Muestra el resultado por stdout y no escribe nada. |

El comando imprime los avisos de migración y deja comentarios `TODO` dentro
del `Jenkinsfile` para todo lo que no tiene equivalente automático. Ver
[Migración desde GitHub Actions](#migración-desde-github-actions).

---

## Cómo funciona por dentro

### Servicio `runit`

`termux-services` usa `runit`. El CLI escribe dos scripts:

`$PREFIX/var/service/jenkins/run`:

```sh
export HOME=...
export PREFIX=...
export JENKINS_HOME=...
export JAVA_HOME="$PREFIX/lib/jvm/java-21-openjdk"
export PATH="$JAVA_HOME/bin:$PREFIX/bin:$PATH"
export FONTCONFIG_PATH="$PREFIX/etc/fonts"
export FONTCONFIG_FILE="$PREFIX/etc/fonts/fonts.conf"

termux-wake-lock 2>/dev/null || true

exec java -Xms256m -Xmx1200m -XX:MaxMetaspaceSize=512m \
  -Djava.awt.headless=true -Dfile.encoding=UTF-8 \
  -Djenkins.install.runSetupWizard=false \
  -jar "$JENKINS_HOME/../jenkins.war" \
  --httpPort=8080 --httpListenAddress=0.0.0.0
```

`$PREFIX/var/service/jenkins/log/run`:

```sh
exec svlogd -tt "$HOME/jenkins/logs"
```

`runit` reinicia el proceso si se cae. `svlogd` rota el log con marca de
tiempo.

### Arranque con Termux:Boot

`~/.termux/boot/10-jenkins.sh`:

```sh
export PATH="$PREFIX/bin:$PATH"
termux-wake-lock
sleep 5
export SVDIR="$PREFIX/var/service"
export LOGDIR="$PREFIX/var/log"
service-daemon start >/dev/null 2>&1 &
sleep 3
sv-enable jenkins >/dev/null 2>&1 || sv start jenkins >/dev/null 2>&1
```

Termux:Boot ejecuta este script al encender el móvil. El `sleep` da tiempo a
que Android monte el almacenamiento y arranque los servicios.

### Java y fuentes

Java en Android necesita `fontconfig` y una fuente real. Sin eso, Jenkins falla
al arrancar con:

```
Fontconfig head is null, check your fonts or fonts configuration
```

El CLI instala `fontconfig`, `fontconfig-utils` y configura
`FONTCONFIG_PATH`/`FONTCONFIG_FILE`. Si usas DejaVu:

```sh
pkg install fontconfig fontconfig-utils
fc-cache -f
```

### Seguridad inicial

`install` escribe `~/jenkins/home/init.groovy.d/00-security.groovy`. En el
primer arranque:

- Crea el usuario `admin` con la contraseña de
  `~/jenkins/home/admin-password.txt`.
- Activa `HudsonPrivateSecurityRealm`.
- Aplica `FullControlOnceLoggedInAuthorizationStrategy`.
- Desactiva la lectura anónima.
- Ajusta ejecutores, mensaje del sistema y URL.

Es idempotente: si la seguridad ya está activada, no vuelve a crear el
usuario.

### Readiness y HTTP

- `install` espera a que `GET /api/json` con `admin:password` devuelva 200
  (hasta 120 s).
- `start`/`restart` esperan a que `GET /login` devuelva 200 (hasta 90 s).
- Durante el arranque es normal ver `503`; Jenkins aún no está listo.

### SIGPIPE

El binario restaura el comportamiento Unix de `SIGPIPE`, así que
`termux-cicd status | head` termina con código 141 sin panic.

---

## Configuración

Archivo: `~/.config/termux-cicd/config.toml`

```toml
port = 8080
heap_mb = 1200
metaspace_mb = 512
executors = 2
jenkins_version = "2.580.2"
url = ""
root = "/data/data/com.termux/files/home/jenkins"
```

| Campo | Significado |
|---|---|
| `port` | Puerto HTTP. |
| `heap_mb` | `-Xmx` de Java. |
| `metaspace_mb` | `-XX:MaxMetaspaceSize`. |
| `executors` | Ejecutores de Jenkins. |
| `jenkins_version` | Última versión instalada por el CLI. |
| `url` | URL pública. Vacío = calcular IP LAN automáticamente. |
| `root` | Directorio de instalación. |

Variables de entorno relevantes:

| Variable | Uso |
|---|---|
| `HOME` | Localizar `~/jenkins` y `~/.config`. |
| `PREFIX` | Localizar Termux (`/data/data/com.termux/files/usr`). |
| `GITHUB_TOKEN` | Token para `github add`/`setup`. |
| `GH_TOKEN` | Alternativa a `GITHUB_TOKEN`. |

---

## Integración con GitHub

### La única conexión es `owner/repo`

El job de Jenkins **no conoce tu directorio local**, ni tu rama de trabajo, ni
tus cambios sin subir. Solo se configura con:

- `owner` y `repo` (por ejemplo `usuario/termux-cicd`).
- Una credencial de GitHub, si el repo es privado.
- La ruta del `Jenkinsfile` dentro del repo (`Jenkinsfile`).
- El intervalo de polling.

```
   Tu proyecto local                GitHub                    Jenkins
 ┌──────────────────┐          ┌──────────────┐          ┌──────────────────┐
 │ git working tree │──push───>│  repositorio │<──clone──│  workspace/...   │
 └──────────────────┘          └──────────────┘   poll   └──────────────────┘
          │                           ▲
          │ solo al configurar        │
          └── termux-cicd ────────────┘
              lee origin, detecta stack,
              genera Jenkinsfile, configura job
```

El directorio local se usa **solo** cuando ejecutas `github add` o `setup`,
para cuatro cosas:

1. Leer `git remote get-url origin` y deducir `owner/repo`.
2. Detectar el stack (`Cargo.toml`, `package.json`, etc.).
3. Generar el `Jenkinsfile` si falta.
4. Con `--push`, hacer commit y push de ese `Jenkinsfile`.

A partir de ahí, Jenkins clona **desde GitHub** en
`~/jenkins/home/workspace/<job>_<rama>` y compila esa copia. Consecuencias:

- Los cambios locales sin `git push` **no existen** para Jenkins.
- Puedes borrar el proyecto local y el job sigue funcionando.
- Si renombras o mueves el repo en GitHub, el job deja de encontrarlo hasta
  que lo actualices.
- Jenkins usa su propia credencial, no tu configuración local de Git ni tu
  sesión de `gh`.
- `--dir` solo afecta a la detección y generación del `Jenkinsfile`; no es una
  ruta de build.
- Aunque el remoto sea SSH (`git@github.com:...`), Jenkins usa la API y el
  clon HTTPS de GitHub a partir de `owner/repo`.

### 1. Deducción del repositorio

Si no pasas `repo`, el CLI ejecuta:

```sh
git -C <dir> rev-parse --show-toplevel
git -C <root> remote get-url origin
```

y acepta:

- `https://github.com/owner/repo`
- `https://github.com/owner/repo.git`
- `http://github.com/owner/repo`
- `git@github.com:owner/repo.git`
- `owner/repo`

Rechaza otros hosts y formatos ambiguos. `--dir` permite apuntar a otro
proyecto sin cambiar de directorio.

### 2. Resolución del token

Orden exacto:

1. `--token-file ARCHIVO` (si se indica y falla, es un error; no continúa).
2. `--token` (o `GITHUB_TOKEN` mediante `clap`).
3. `GITHUB_TOKEN`.
4. `GH_TOKEN`.
5. `~/.config/termux-cicd/github-token`
6. `~/.config/github-token`
7. `~/.github-token`
8. `~/.config/gh/hosts.yml` (campo `oauth_token`).
9. `gh auth token`.

Si no hay token, el job se configura para acceso anónimo. Sirve para repos
públicos; para privados necesitas token.

Usuarios de GitHub:

- Por defecto `x-access-token`, que funciona con tokens de GitHub App y con
  muchos PAT.
- Si tu PAT clásico o fine-grained falla, prueba con tu usuario real:

  ```sh
  termux-cicd github add usuario/repo --user TU_USUARIO
  ```

Permisos mínimos recomendados:

- PAT clásico: `repo` (privados) o `public_repo` (públicos).
- Fine-grained: acceso al repositorio, `Contents: read`, `Metadata: read` y
  `Pull requests: read` si quieres descubrir PRs.

### 3. Credencial en Jenkins

`github add` crea o actualiza una credencial *Username with password*:

- ID: `github-pat` (o `--credentials-id`).
- Usuario: `--user` (por defecto `x-access-token`).
- Contraseña: el token.
- Ámbito: global.

Si ya existe una credencial con ese ID, se actualiza en lugar de duplicarse.

### 4. Multibranch Pipeline

El job se crea con:

- Tipo: `WorkflowMultiBranchProject` (Multibranch Pipeline).
- Branch Source: `GitHubSCMSource`.
- Descubrimiento de ramas: todas, excluyendo PRs como ramas.
- Descubrimiento de PRs: PRs del origen fusionados con el destino.
- `Jenkinsfile` en la raíz del repo (`scriptPath = Jenkinsfile`).

Si ya existe un item con el mismo nombre y **no** es un Multibranch Pipeline,
el comando falla en lugar de sobrescribirlo.

### 5. Polling

Sin IP pública no hay webhooks. El CLI usa
`PeriodicFolderTrigger` con un intervalo:

- `--poll-minutes 5` → `5m` → `300000 ms`.
- `--poll-minutes 0` → sin trigger.
- Rango válido: 1–60 minutos.

El intervalo se guarda en `config.xml` y Jenkins escanea el repo aunque no
reciba eventos de GitHub.

### 6. Jenkinsfile automático

Si el proyecto local no tiene `Jenkinsfile`, `github add` y `setup` intentan
primero **migrar los workflows de GitHub Actions**. Si no hay
`.github/workflows` o ningún paso es convertible, se genera una plantilla
según el primer archivo detectado:

| Archivo | Stack | Stages |
|---|---|---|
| `Cargo.toml` | Rust | `cargo build --release`, `cargo test` |
| `package.json` | Node.js | `npm ci`/`npm install`, `npm test`, `npm run build` |
| `pyproject.toml`, `requirements.txt`, `setup.py` | Python | venv + `pytest`/`unittest` |
| `pom.xml` | Maven | `./mvnw -B verify` o `mvn -B verify` |
| `build.gradle`, `build.gradle.kts` | Gradle | `./gradlew build` o `gradle build` |
| `go.mod` | Go | `go test ./...`, `go build ./...` |
| ninguno | Genérico | inspección del repo |

La prioridad es:

1. `migrate` de `.github/workflows` si hay pasos convertibles.
2. Plantilla por stack si no hay workflows o no se pudo convertir nada.

Todos los pipelines incluyen:

- `timestamps()` (plugin Timestamper).
- `disableConcurrentBuilds()`.
- Retención de 10 builds.
- `cleanWs()` al final (plugin Workspace Cleanup).

Con `--push`, el CLI hace `git add Jenkinsfile`, commit y push. Si no hay
identidad de Git configurada, usa una de relleno solo para ese commit. El push
se ejecuta con `GIT_TERMINAL_PROMPT=0` para no quedarse esperando credenciales.

### 7. `github list`

Muestra:

```
JOB             REPO                          RAMAS  ULTIMO SCAN
termux-cicd     usuario/termux-cicd           1      SUCCESS
```

Solo lista `WorkflowMultiBranchProject`, no pipelines simples.

### 8. Qué pasa en el escaneo

- Si el repo tiene `Jenkinsfile`, se crea un job por rama y se compila.
- Si no lo tiene, el escaneo termina `SUCCESS` con 0 ramas y el log de
  indexación indica `'Jenkinsfile' not found`.
- Si el repo es privado y el token no sirve, el escaneo falla con error de
  autenticación.

---

## Migración desde GitHub Actions

Jenkins **no lee** `.github/workflows`. Si un proyecto ya tiene CI en GitHub
Actions, `termux-cicd migrate` genera un `Jenkinsfile` a partir de esos YAML.

### Por qué un conversor propio

Antes de implementarlo se revisaron las alternativas open source disponibles:

| Herramienta | Dirección | Limitación |
|---|---|---|
| `gh actions-importer` (GitHub) | Jenkins/Azure/GitLab → **GitHub Actions** | Va en sentido contrario. |
| `Huckcity/jenkins-to-actions` | **Jenkinsfile → GitHub Actions** | También en sentido contrario. |
| `bk pipeline convert` (Buildkite) | GitHub Actions → **Buildkite** | No genera Jenkins. |
| Conversor web de infrafolks | GitHub Actions → Jenkinsfile | Solo estructural; las acciones del Marketplace requieren reemplazo manual. No es un CLI instalable ni mantenible. |

No existe una herramienta open source madura y mantenida que convierta
GitHub Actions a `Jenkinsfile`. Por eso el proyecto incluye un conversor
**conservador**: traduce lo que tiene equivalencia clara y marca con `TODO`
todo lo que necesita decisión humana. No inventa pasos ni oculta diferencias.

### Qué se convierte

| GitHub Actions | Jenkinsfile |
|---|---|
| `jobs` | `stage(...)` |
| `needs` | Orden de los stages (topológico). |
| `run` | `sh '''...'''` |
| `env` de workflow y job | `environment { ... }` |
| `env` de paso | `withEnv([...])` |
| `working-directory` | `dir('...') { ... }` |
| `actions/checkout` | Se omite: Jenkins hace el checkout del SCM. |
| `actions/upload-artifact` | `archiveArtifacts artifacts: '...'` |
| `timeout-minutes` de job | `options { timeout(...) }` |
| `${{ secrets.X }}` | `X = credentials('X')` + `${X}` en el script |
| `${{ github.sha }}` | `${GIT_COMMIT}` |
| `${{ github.ref_name }}` | `${BRANCH_NAME}` |
| `${{ github.workspace }}` | `${WORKSPACE}` |
| `${{ runner.os }}` | `Linux` |
| `${{ env.X }}` | `${X}` |
| Varios workflows | Stages con prefijo `<workflow> / <job>`. |
| Scripts multilínea y heredocs | Se conservan literalmente. |

El resultado incluye `timestamps()`, `disableConcurrentBuilds()`, retención de
10 builds y `cleanWs()`, igual que las plantillas por stack.

### Qué queda en `TODO`

- `actions/setup-node`, `setup-python`, `setup-java`, `setup-go`, etc.: el
  toolchain debe existir en Termux.
- Acciones del Marketplace sin equivalente: Docker, caché, releases,
  `github-script`, despliegues, etc.
- `actions/download-artifact`: requiere el plugin Copy Artifact u otra forma
  de pasar artefactos entre jobs.
- `strategy.matrix`: se genera una sola ejecución y se avisa.
- `if:`: no se traduce la sintaxis de expresiones de GitHub; el paso se
  ejecuta siempre y se marca con `TODO`.
- `shell: pwsh` o `cmd`: el script se deja comentado.
- `continue-on-error`, `timeout-minutes` por paso, `services`, `container`,
  `permissions`, `concurrency` y workflows reutilizables.
- Triggers `on:`: el job usa polling de termux-cicd.

### Ejemplo

```sh
cd mi-proyecto
termux-cicd migrate --dry-run     # revisar sin escribir
termux-cicd migrate               # crea Jenkinsfile
```

Salida resumida:

```
Workflows encontrados: ci.yml

Avisos de migración (6):
  - Los triggers `on:` de GitHub Actions no se migran...
  - `actions/setup-node@v4` no tiene equivalente automático...
  - secreto `NPM_TOKEN` mapeado a la credencial `NPM_TOKEN`...

Jenkinsfile escrito en /home/.../Jenkinsfile (2 stages migrados).
Revisa los TODO y los avisos antes de hacer commit y push.
```

Fragmento generado:

```groovy
stage('test') {
    environment {
        CARGO_TERM_COLOR = 'always'
    }
    steps {
        // actions/checkout: el checkout lo hace Jenkins desde el SCM
        // TODO: reemplazar actions/setup-node@v4 (instala el toolchain...)
        sh '''
npm ci
        '''
        dir('web') {
            sh '''
npm test
            '''
        }
        archiveArtifacts artifacts: 'web/coverage/**', allowEmptyArchive: true
    }
}
```

### Límites y validación

- El conversor **no garantiza equivalencia funcional**. GitHub Actions y
  Jenkins tienen modelos distintos de runners, permisos, caché y secretos.
- Los secretos se convierten en referencias a credenciales de Jenkins. El
  valor del token **nunca** se escribe en el `Jenkinsfile`; hay que crear cada
  credencial (tipo *Secret text*) con el mismo ID.
- Los scripts se emiten sin sangría dentro de `sh '''...'''` para no romper
  heredocs.
- El resultado se valida con el endpoint
  `pipeline-model-converter/validate` de Jenkins en las pruebas del proyecto.
- Después de migrar: revisa los `TODO`, ejecuta el pipeline, ajusta rutas y
  toolchains, y solo entonces haz `git push`.

Si `github add` o `setup` encuentran workflows pero no logran convertir
ningún paso, avisan y usan la plantilla por stack en lugar de dejar un
`Jenkinsfile` vacío.

---

## Estructura en disco

| Ruta | Contenido |
|---|---|
| `~/jenkins/jenkins.war` | Jenkins LTS |
| `~/jenkins/jenkins.war.<v>.bak` | WAR anterior tras `update` |
| `~/jenkins/home` | `JENKINS_HOME`: jobs, plugins, credenciales |
| `~/jenkins/home/admin-password.txt` | Contraseña de `admin` (`0600`) |
| `~/jenkins/home/init.groovy.d/00-security.groovy` | Seguridad inicial |
| `~/jenkins/plugins.txt` | Lista de plugins |
| `~/jenkins/jenkins-plugin-manager.jar` | Plugin Manager |
| `~/jenkins/jenkins-cli.jar` | CLI de Jenkins (bajo demanda) |
| `~/jenkins/url.txt` | URL configurada |
| `~/jenkins/.version` | Versión instalada |
| `~/jenkins/logs/current` | Log del servicio |
| `~/.config/termux-cicd/config.toml` | Configuración del CLI |
| `$PREFIX/var/service/jenkins/` | Servicio `runit` |
| `~/.termux/boot/10-jenkins.sh` | Arranque con Termux:Boot |

---

## Seguridad

### Contraseña de Jenkins

Se genera con `/dev/urandom` y 24 caracteres. Se guarda en
`admin-password.txt` con permisos `0600`. Aparece en la salida de `install`;
no la publiques en capturas ni logs.

### Exposición de red

Jenkins escucha en `0.0.0.0:8080`. Eso significa **todas** las interfaces,
incluida la Wi-Fi local. No lo expongas a Internet:

- No hagas port forwarding en el router.
- No uses ngrok/Cloudflare Tunnel sin autenticación adicional.
- Usa Tailscale o una LAN de confianza.
- El CLI no expone una opción para cambiar el bind; si necesitas restringirlo
  a `127.0.0.1`, edita el script del servicio manualmente.

### Token de GitHub

- `--token` queda visible en `ps` e historial; prefiere `GITHUB_TOKEN`,
  `--token-file` o `gh auth token`.
- El token se envía a Jenkins por **stdin** en un script Groovy, nunca como
  argumento de proceso.
- Jenkins lo guarda cifrado en su almacén de credenciales, pero sigue siendo
  un secreto: protege `JENKINS_HOME` y los backups.
- Un token de `gh` (`gho_...`) es válido, pero para CI a largo plazo es mejor
  un PAT específico con permisos mínimos y rotación.

### Contraseña del CLI de Jenkins

Para ejecutar Groovy, el CLI crea `~/jenkins/.cli-auth.tmp` con `0600`,
ejecuta `jenkins-cli.jar -auth @archivo` y lo borra al terminar. Así la
contraseña no aparece en `ps`.

### Backups

Un `.tgz` de `~/jenkins` contiene credenciales, claves y la contraseña de
`admin`. Guárdalo cifrado y fuera de repositorios públicos.

---

## Acceso remoto

### Tailscale (recomendado)

La app oficial de Tailscale para Android funciona sin root y sin exponer
puertos. Pasos:

1. Instala Tailscale en el móvil y en el equipo desde el que quieras acceder.
2. Inicia sesión en la misma tailnet.
3. Averigua la IP `100.x.y.z` del móvil.
4. Abre `http://100.x.y.z:8080`.

También puedes usar MagicDNS:

```sh
termux-cicd config set url http://mi-movil.tailnet.ts.net:8080/
```

La URL debe coincidir exactamente con la que usa Jenkins; si no, el CLI de
Jenkins falla con `Unexpected request origin`.

### LAN

`termux-cicd status` muestra la IP LAN detectada. Úsala solo en redes de
confianza.

### Termux:Boot

Instala la app **Termux:Boot** desde F-Droid/GitHub, ábrela una vez y
desactiva la optimización de batería de Termux:

```
Ajustes de Android → Aplicaciones → Termux → Batería → Sin restricciones
```

Sin eso, Android puede matar Jenkins al apagar la pantalla o al reiniciar.

---

## Pruebas

### Pruebas unitarias

```sh
cargo test
```

Cubren:

- Parseo de repositorios HTTPS, SSH y `owner/repo`.
- Rechazo de formatos inválidos.
- Detección de repo desde `git remote origin`.
- Error claro cuando no hay remoto.
- Detección de stack y generación de `Jenkinsfile`.
- Detección de Rust, Node, Python, Maven, Gradle y Go.
- Generación de contraseñas y formato de tamaños.
- Migración de workflows:
  - `run` → `sh` y omisión de `actions/checkout`.
  - Orden de jobs según `needs`.
  - `env` de workflow/job y `working-directory`.
  - `upload-artifact` → `archiveArtifacts`.
  - Avisos de `setup-*`, matrices y expresiones no portables.
  - Secretos → credenciales de Jenkins.
  - Escape de comillas, barras invertidas y `'''`.
  - Varios workflows con prefijo de stage.
  - Detección de workflows sin pasos soportados.
  - Prioridad de `.github/workflows` sobre la plantilla por stack y fallback.
  - Secretos en `env` que generan credenciales sin comillas extra.

Salida esperada:

```
running 23 tests
...
test result: ok. 23 passed; 0 failed
```

### Compilación de release

```sh
cargo build --release
ls -lh target/release/termux-cicd
```

El binario ronda 1.6 MB y no depende de archivos externos.

### Checklist manual en el móvil

Ejecuta en orden y comprueba el resultado:

| # | Comando | Resultado esperado |
|---|---|---|
| T1 | `termux-cicd --version` | `termux-cicd 0.1.0` |
| T2 | `termux-cicd doctor` | 11 ✔ y aviso de Termux:Boot si falta |
| T3 | `termux-cicd status` | `EN EJECUCIÓN`, HTTP 200, PID y versión |
| T4 | `termux-cicd logs -n 5` | Últimas líneas del log |
| T5 | `termux-cicd logs -f` | Sigue el log; `Ctrl+C` sale |
| T6 | `termux-cicd restart` | Reinicia y vuelve a HTTP 200 |
| T7 | `termux-cicd stop` | `DETENIDO` |
| T8 | `termux-cicd start` | `EN EJECUCIÓN` y HTTP 200 |
| T9 | `curl -u admin:$(cat ~/jenkins/home/admin-password.txt) http://127.0.0.1:8080/api/json` | JSON con `jobs` |
| T10 | `termux-cicd status \| head -3` | Sin panic; código 141 aceptable |
| T11 | `termux-cicd config show` | TOML con los valores actuales |
| T12 | `termux-cicd config set executors 2` | Aplica y reinicia si estaba activo |
| T13 | `termux-cicd migrate --dry-run` en un proyecto con `.github/workflows` | Muestra el `Jenkinsfile` sin escribir |
| T14 | `termux-cicd migrate` dos veces | La segunda falla y pide `--force` |

### Prueba de instalación desde cero

Sirve para validar el flujo completo en un directorio y puerto alternativos.

> **Aviso:** el servicio se llama siempre `jenkins` en
> `$PREFIX/var/service`. Una instalación con `--root`/`--port` alternativos
> **reemplaza** el servicio activo y el script de Termux:Boot. Al terminar,
> reinstala la principal.

```sh
termux-cicd --root ~/tmp/jenkins-test --port 8081 install
termux-cicd --root ~/tmp/jenkins-test --port 8081 status
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8081/login
termux-cicd --root ~/tmp/jenkins-test --port 8081 uninstall --purge
rm -rf ~/tmp/jenkins-test
termux-cicd --root ~/jenkins --port 8080 install   # recuperar la principal
termux-cicd status
```

Resultado esperado: HTTP 200 en 8081, uninstall sin errores y la instalación
principal de vuelta en 8080.

### Prueba de backup y restore

```sh
termux-cicd backup --output ~/tmp/jenkins-test.tgz
ls -lh ~/tmp/jenkins-test.tgz
tar tzf ~/tmp/jenkins-test.tgz | grep -c '/home/war/' || true
termux-cicd restore ~/tmp/jenkins-test.tgz
termux-cicd status
```

Resultado esperado:

- El backup pesa ~195 MB con una instalación LTS + 12 plugins.
- `grep -c '/home/war/'` devuelve `0` (excluido).
- Tras `restore`, Jenkins vuelve a HTTP 200 y conserva los jobs.

### Prueba de GitHub con repo público

```sh
termux-cicd github add https://github.com/jenkins-docs/simple-java-maven-app \
  --name prueba-cicd --no-scan
termux-cicd github list
```

Resultado esperado:

- Se crea el job `prueba-cicd`.
- `github list` muestra el repo y `sin scan`.
- El `config.xml` del job contiene un `PeriodicFolderTrigger`.

Luego lanza un escaneo y observa:

```sh
termux-cicd github list
# tras unos segundos: ULTIMO SCAN = SUCCESS, RAMAS = 0
# porque ese repo no tiene Jenkinsfile
```

### Prueba de `setup` sin argumentos

```sh
mkdir -p ~/tmp/proyecto-demo && cd ~/tmp/proyecto-demo
git init -q
git remote add origin https://github.com/jenkins-docs/simple-java-maven-app.git
printf '<project/>\n' > pom.xml
termux-cicd setup --name prueba-setup
test -f Jenkinsfile && grep -q 'mvn' Jenkinsfile && echo "Jenkinsfile Maven OK"
termux-cicd github list
```

Resultado esperado:

- Detecta `jenkins-docs/simple-java-maven-app`.
- Genera un `Jenkinsfile` con `mvn`.
- Crea el job y lanza el escaneo.

### Prueba de migración de GitHub Actions

```sh
mkdir -p ~/tmp/proyecto-gha/.github/workflows && cd ~/tmp/proyecto-gha
cat > .github/workflows/ci.yml <<'EOF'
name: CI
on: [push]
jobs:
  test:
    runs-on: ubuntu-latest
    env:
      NODE_ENV: test
    steps:
      - uses: actions/checkout@v4
      - run: npm ci
      - run: npm test
        working-directory: web
      - uses: actions/upload-artifact@v4
        with:
          name: cobertura
          path: web/coverage/**
  deploy:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - run: echo "token ${{ secrets.NPM_TOKEN }}"
EOF

termux-cicd migrate --dry-run > ~/tmp/jenkinsfile.preview
grep -q "stage('test')" ~/tmp/jenkinsfile.preview && echo "dry-run OK"

termux-cicd migrate
grep -q "archiveArtifacts" Jenkinsfile && echo "artefactos OK"
grep -q "NPM_TOKEN = credentials('NPM_TOKEN')" Jenkinsfile && echo "secretos OK"
grep -q "dir('web')" Jenkinsfile && echo "working-directory OK"

# La segunda ejecución no debe sobrescribir sin --force
termux-cicd migrate; echo "código esperado 1: $?"
```

Validación real con Jenkins:

```sh
PW=$(cat ~/jenkins/home/admin-password.txt)
CRUMB=$(curl -s -u "admin:$PW" http://127.0.0.1:8080/crumbIssuer/api/json \
  | sed -n 's/.*"crumb":"\([^"]*\)".*/\1/p')
curl -s -u "admin:$PW" -H "Jenkins-Crumb: $CRUMB" \
  --data-urlencode "jenkinsfile@Jenkinsfile" \
  http://127.0.0.1:8080/pipeline-model-converter/validate
rm -rf ~/tmp/proyecto-gha ~/tmp/jenkinsfile.preview
```

Resultado esperado: `Jenkinsfile successfully validated.` y los cuatro `OK`.

### Limpieza de jobs y credenciales de prueba

Requiere que `jenkins-cli.jar` exista; se descarga la primera vez que ejecutas
`termux-cicd github add` o `termux-cicd github list`.

```sh
PW=$(cat ~/jenkins/home/admin-password.txt)
printf 'admin:%s\n' "$PW" > ~/tmp/cli-auth
chmod 600 ~/tmp/cli-auth

cat > ~/tmp/cleanup.groovy <<'EOF'
import jenkins.model.Jenkins
import com.cloudbees.plugins.credentials.SystemCredentialsProvider
import com.cloudbees.plugins.credentials.domains.Domain

['prueba-cicd', 'prueba-setup'].each { name ->
    def job = Jenkins.get().getItem(name)
    if (job != null) { job.delete(); println "job eliminado: ${name}" }
}

def store = SystemCredentialsProvider.getInstance().getStore()
store.getCredentials(Domain.global()).findAll { it.id == 'github-pat' }.each {
    store.removeCredentials(Domain.global(), it)
    println "credencial eliminada: ${it.id}"
}
EOF

java -jar ~/jenkins/jenkins-cli.jar \
  -s "$(cat ~/jenkins/url.txt)" \
  -auth @$HOME/tmp/cli-auth \
  groovy = < ~/tmp/cleanup.groovy

rm -f ~/tmp/cli-auth ~/tmp/cleanup.groovy
rm -rf ~/tmp/proyecto-demo
```

> No ejecutes la limpieza de `github-pat` si esa credencial es la que usa tu
> job real.

### Comprobar que no hay secretos en el repo

```sh
git grep -nE 'ghp_|github_pat_|gho_|BEGIN (RSA|OPENSSH|EC) PRIVATE KEY' || echo "sin secretos"
```

---

## Solución de problemas

### Jenkins no arranca

```sh
termux-cicd status
termux-cicd logs -n 100
SVDIR=$PREFIX/var/service LOGDIR=$PREFIX/var/log sv status jenkins
```

Causas habituales:

- Falta `fontconfig` o una fuente → `Fontconfig head is null`.
- Puerto ocupado → `config set port 8081`.
- Poca memoria → reduce `heap_mb` y `executors`.
- Servicio sin permiso de ejecución → revisa `$PREFIX/var/service/jenkins/run`.

### `Fontconfig head is null`

```sh
pkg install fontconfig fontconfig-utils
fc-cache -f
termux-cicd restart
```

El servicio ya exporta `FONTCONFIG_PATH` y `FONTCONFIG_FILE`.

### `Unexpected request origin`

El CLI de Jenkins exige que `-s` coincida con la URL configurada en Jenkins.
Comprueba:

```sh
cat ~/jenkins/url.txt
termux-cicd config show
```

Si cambias de IP o de Tailscale, actualiza:

```sh
termux-cicd config set url http://nueva-ip:8080/
```

### HTTP 503 al arrancar

Es normal durante los primeros segundos. Espera o mira el log:

```sh
termux-cicd logs -f
```

### `sv: command not found`

Falta `termux-services`:

```sh
pkg install termux-services
termux-cicd install
```

### Jenkins se muere con la pantalla apagada

- Comprueba que el script `run` ejecuta `termux-wake-lock`.
- Desactiva la optimización de batería para Termux.
- Instala Termux:Boot si quieres arranque tras reinicio.

### El escaneo de GitHub descubre 0 ramas

- El repo no tiene `Jenkinsfile` en la rama.
- La rama por defecto no es la esperada.
- El repo es privado y el token no tiene permisos.
- GitHub está limitando peticiones (rate limit).

Revisa:

```sh
tail -50 ~/jenkins/home/jobs/<job>/indexing/indexing.log
```

### Error de autenticación de GitHub

```sh
gh auth status
gh auth token >/dev/null && echo "token disponible"
```

Para PAT clásico prueba `--user TU_USUARIO`. Para fine-grained, revisa
`Contents: read`, `Metadata: read` y `Pull requests: read`.

### `git push` del Jenkinsfile falla

El CLI usa `GIT_TERMINAL_PROMPT=0`, así que no se queda colgado: falla y te
avisa. Configura credenciales de Git o sube el archivo manualmente:

```sh
git add Jenkinsfile
git commit -m "ci: añadir Jenkinsfile"
git push
```

### No hay espacio en disco

```sh
termux-cicd backup --output /sdcard/jenkins.tgz
termux-cicd uninstall --purge
```

Los backups y `JENKINS_HOME` son los que más ocupan. Revisa también
`~/jenkins/home/war`.

---

## Limitaciones y no objetivos

- **Un solo Jenkins activo por Termux.** El servicio se llama siempre
  `jenkins`; `--root`/`--port` permiten datos alternativos, pero el servicio
  activo es uno.
- **Sin webhooks.** Requiere IP pública o un túnel; el diseño usa polling.
- **Sin TLS.** No hay HTTPS integrado; usa Tailscale o un proxy delante.
- **Sin multi-nodo.** Es un servidor Jenkins de un solo nodo Android.
- **Jenkins clona desde GitHub, no desde tu disco.** El job solo conoce
  `owner/repo`; el directorio local se usa para configurar, no para compilar.
- **Sin sandbox de builds.** Los pipelines ejecutan comandos como tu usuario
  de Termux, con acceso a `$HOME`.
- **Sin gestor de toolchains.** No instala Node, Python, Go, etc.; solo
  Jenkins. Los `Jenkinsfile` asumen que esas herramientas existen.
- **Migración conservadora.** `migrate` traduce solo lo que tiene equivalencia
  clara. Las acciones del Marketplace, matrices y expresiones de GitHub
  quedan marcadas con `TODO`; la equivalencia funcional no está garantizada.
- **Sin CI de GitHub Actions en este repo.** Las pruebas requieren Termux/
  Android y se ejecutan localmente.
- **Sin soporte oficial de otras plataformas.** El código asume Termux,
  `pkg`, `sv` y `termux-wake-lock`.

---

## Roadmap

- [x] MVP de instalación y ciclo de vida.
- [x] Backup/restore consistente.
- [x] Comando `github` con credenciales y Multibranch Pipeline.
- [x] Detección automática de repo, token y stack.
- [x] Generación de `Jenkinsfile` por stack.
- [x] Migración de `.github/workflows` a `Jenkinsfile` (`migrate`).
- [ ] Perfiles de plugins por stack (Node, Python, Rust, Go...).
- [ ] Instalación de toolchains de build (`nodejs`, `python`, `golang`).
- [ ] `doctor --fix`.
- [ ] Soporte de webhooks mediante túnel opcional.
- [ ] Tests de integración automatizados en Termux.
- [ ] Empaquetado en `.deb` de Termux o instalador de una línea.

---

## Contribuir

Las contribuciones son bienvenidas, especialmente si has probado el proyecto
en otro Android, arquitectura o versión de Jenkins.

### Antes de abrir un PR

```sh
cargo fmt
cargo test
cargo build --release
```

### Convenciones

- Salida de usuario y documentación en **español**.
- Código y nombres de funciones en inglés, consistente con el resto.
- Commits estilo Conventional Commits: `feat:`, `fix:`, `docs:`, `test:`,
  `refactor:`.
- No incluyas `target/`, backups, WARs, JARs ni credenciales.
- Si añades un comando, actualiza README y añade al menos una prueba.

### Reportar un bug

Incluye:

- Modelo de móvil, versión de Android y arquitectura (`uname -m`).
- Versión de Termux, Java y Jenkins.
- Salida de `termux-cicd doctor`.
- Salida de `termux-cicd logs -n 100`.
- Pasos exactos para reproducirlo.

No adjuntes tokens, contraseñas ni backups.

---

## Licencia

MIT. Consulta [LICENSE](LICENSE).
