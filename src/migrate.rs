use anyhow::{bail, Context, Result};
use serde_yaml::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Resultado de convertir uno o varios workflows de GitHub Actions.
pub struct Generated {
    pub content: String,
    pub workflows: Vec<String>,
    pub warnings: Vec<String>,
    pub stages: usize,
    pub supported_steps: usize,
}

impl Generated {
    pub fn usable(&self) -> bool {
        self.supported_steps > 0
    }
}

/// Comando `termux-cicd migrate`.
pub fn run(dir: &Path, output: Option<PathBuf>, force: bool, dry_run: bool) -> Result<()> {
    let dir = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(dir)
    };

    let output = output.unwrap_or_else(|| dir.join("Jenkinsfile"));
    if !dry_run && output.exists() && !force {
        bail!(
            "{} ya existe. Usa --force para sobrescribirlo o --output para elegir otro archivo.",
            output.display()
        );
    }

    let generated = match generate(&dir)? {
        Some(generated) => generated,
        None => bail!(
            "no hay workflows en {}. Crea .github/workflows/*.yml o indica otro directorio con --dir.",
            dir.join(".github/workflows").display()
        ),
    };

    println!("Workflows encontrados: {}", generated.workflows.join(", "));
    if !generated.warnings.is_empty() {
        println!("\nAvisos de migración ({}):", generated.warnings.len());
        for warning in &generated.warnings {
            println!("  - {warning}");
        }
    }

    if !generated.usable() {
        bail!(
            "ningún workflow contiene pasos convertibles. No se ha escrito nada:\n\
             GitHub Actions y Jenkins no son equivalentes; revisa los avisos."
        );
    }

    if dry_run {
        println!("\n--- Jenkinsfile (simulación, no se escribe nada) ---\n");
        print!("{}", generated.content);
        return Ok(());
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("no se pudo crear {}", parent.display()))?;
        }
    }
    fs::write(&output, &generated.content)
        .with_context(|| format!("no se pudo escribir {}", output.display()))?;

    println!(
        "\nJenkinsfile escrito en {} ({} stages migrados).",
        output.display(),
        generated.stages
    );
    println!("Revisa los TODO y los avisos antes de hacer commit y push.");
    Ok(())
}

/// Convierte los workflows de `root/.github/workflows`.
///
/// Devuelve `None` si no existe el directorio o no hay archivos `.yml`/`.yaml`.
pub fn generate(root: &Path) -> Result<Option<Generated>> {
    let workflows_dir = root.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(None);
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&workflows_dir)
        .with_context(|| format!("no se pudo leer {}", workflows_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "yml" | "yaml"))
                    .unwrap_or(false)
        })
        .collect();
    files.sort();

    if files.is_empty() {
        return Ok(None);
    }

    let mut workflows = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for file in &files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workflow")
            .to_string();
        match parse_workflow(file, name.clone()) {
            Ok(workflow) => workflows.push(workflow),
            Err(err) => errors.push(format!(".github/workflows/{name}: {err:#}")),
        }
    }

    warnings.extend(errors);

    if workflows.is_empty() {
        bail!(
            "no he podido leer ningún workflow:\n  - {}",
            warnings.join("\n  - ")
        );
    }

    render(workflows, warnings).map(Some)
}

struct Workflow {
    file: String,
    env: BTreeMap<String, String>,
    jobs: Vec<Job>,
}

struct Job {
    id: String,
    name: Option<String>,
    needs: Vec<String>,
    env: BTreeMap<String, String>,
    runs_on: Option<String>,
    condition: Option<String>,
    timeout_minutes: Option<u64>,
    matrix: Option<String>,
    steps: Vec<Step>,
    reusable: Option<String>,
    extra: Vec<String>,
}

#[derive(Default)]
struct Step {
    name: Option<String>,
    run: Option<String>,
    uses: Option<String>,
    with: BTreeMap<String, String>,
    env: BTreeMap<String, String>,
    working_directory: Option<String>,
    shell: Option<String>,
    condition: Option<String>,
    continue_on_error: bool,
    timeout_minutes: Option<u64>,
}

fn parse_workflow(path: &Path, file: String) -> Result<Workflow> {
    let text =
        fs::read_to_string(path).with_context(|| format!("no se pudo leer {}", path.display()))?;
    let value: Value = serde_yaml::from_str(&text)
        .with_context(|| format!("YAML inválido en {}", path.display()))?;
    let root = value
        .as_mapping()
        .with_context(|| format!("{} no contiene un objeto YAML en la raíz", path.display()))?;

    let env = string_map(field(root, "env"));
    let jobs_value = field(root, "jobs")
        .and_then(|value| value.as_mapping())
        .with_context(|| format!("{} no tiene una sección `jobs` válida", path.display()))?;

    let mut jobs = Vec::new();
    for (key, value) in jobs_value {
        let id = scalar(key).unwrap_or_else(|| "job".to_string());
        let map = match value.as_mapping() {
            Some(map) => map,
            None => continue,
        };
        jobs.push(parse_job(id, map));
    }
    if jobs.is_empty() {
        bail!("{} no contiene jobs", path.display());
    }

    Ok(Workflow { file, env, jobs })
}

fn parse_job(id: String, map: &Mapping) -> Job {
    let mut extra = Vec::new();
    for key in [
        "services",
        "container",
        "permissions",
        "concurrency",
        "outputs",
    ] {
        if field(map, key).is_some() {
            extra.push(key.to_string());
        }
    }

    let matrix = field(map, "strategy")
        .and_then(|value| value.as_mapping())
        .and_then(|strategy| field(strategy, "matrix"))
        .map(matrix_summary);

    let steps = field(map, "steps")
        .and_then(|value| value.as_sequence())
        .map(|sequence| {
            sequence
                .iter()
                .filter_map(|value| value.as_mapping().map(parse_step))
                .collect()
        })
        .unwrap_or_default();

    Job {
        id,
        name: field(map, "name").and_then(scalar),
        needs: string_list(field(map, "needs")),
        env: string_map(field(map, "env")),
        runs_on: field(map, "runs-on").and_then(|value| {
            value
                .as_sequence()
                .map(|sequence| {
                    sequence
                        .iter()
                        .filter_map(scalar)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .or_else(|| scalar(value))
        }),
        condition: field(map, "if").and_then(scalar),
        timeout_minutes: field(map, "timeout-minutes").and_then(|value| value.as_u64()),
        matrix,
        steps,
        reusable: field(map, "uses").and_then(scalar),
        extra,
    }
}

fn parse_step(map: &Mapping) -> Step {
    Step {
        name: field(map, "name").and_then(scalar),
        run: field(map, "run").and_then(scalar),
        uses: field(map, "uses").and_then(scalar),
        with: string_map(field(map, "with")),
        env: string_map(field(map, "env")),
        working_directory: field(map, "working-directory").and_then(scalar),
        shell: field(map, "shell").and_then(scalar),
        condition: field(map, "if").and_then(scalar),
        continue_on_error: field(map, "continue-on-error")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        timeout_minutes: field(map, "timeout-minutes").and_then(|value| value.as_u64()),
    }
}

fn render(workflows: Vec<Workflow>, mut warnings: Vec<String>) -> Result<Generated> {
    let multiple = workflows.len() > 1;
    let mut stages: Vec<Vec<Line>> = Vec::new();
    let mut stage_count = 0usize;
    let mut supported_steps = 0usize;
    let mut any_needs = false;
    let mut any_runs_on = false;

    warnings.push(
        "Los triggers `on:` de GitHub Actions no se migran: el job se dispara con el polling de termux-cicd (o lánzalo manualmente)."
            .to_string(),
    );

    for workflow in &workflows {
        let (order, order_warnings) = order_jobs(&workflow.jobs);
        warnings.extend(order_warnings);
        if workflow.jobs.iter().any(|job| !job.needs.is_empty()) {
            any_needs = true;
        }
        if workflow.jobs.iter().any(|job| job.runs_on.is_some()) {
            any_runs_on = true;
        }

        for index in order {
            let job = &workflow.jobs[index];
            if job.reusable.is_some() {
                push_unique(
                    &mut warnings,
                    format!(
                        "{} / {}: workflow reutilizable (`uses: {}`) no soportado; créalo a mano.",
                        workflow.file,
                        job.id,
                        job.reusable.as_deref().unwrap_or("")
                    ),
                );
                continue;
            }

            for extra in &job.extra {
                push_unique(
                    &mut warnings,
                    format!(
                        "{} / {}: la clave `{extra}` no tiene equivalente directo; revísala.",
                        workflow.file, job.id
                    ),
                );
            }
            if let Some(matrix) = &job.matrix {
                push_unique(
                    &mut warnings,
                    format!(
                        "{} / {}: matriz {matrix} no soportada; se genera una sola ejecución con TODO.",
                        workflow.file, job.id
                    ),
                );
            }

            let mut creds = BTreeSet::new();
            let mut body = Vec::new();
            let base = 16usize;
            let mut job_supported = false;

            for step in &job.steps {
                let (lines, supported) = render_step(step, base, &mut creds, &mut warnings);
                body.extend(lines);
                job_supported |= supported;
            }

            if body.is_empty() {
                push_unique(
                    &mut warnings,
                    format!(
                        "{} / {}: no tiene pasos convertibles; se omite el stage.",
                        workflow.file, job.id
                    ),
                );
                continue;
            }
            if job_supported {
                supported_steps += 1;
            }

            let stage_env = merged_env(&workflow.env, &job.env, &mut creds, &mut warnings);
            let stage_name = job.name.clone().unwrap_or_else(|| job.id.clone());
            let stage_name = if multiple {
                format!(
                    "{} / {}",
                    Path::new(&workflow.file)
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or(&workflow.file),
                    stage_name
                )
            } else {
                stage_name
            };

            let mut lines = Vec::new();
            lines.push(code_line(8, format!("stage({}) {{", quote(&stage_name))));

            if let Some(condition) = &job.condition {
                lines.push(code_line(
                    12,
                    format!(
                        "// TODO: condición de GitHub Actions no portable: {}",
                        one_line(condition)
                    ),
                ));
            }
            if let Some(matrix) = &job.matrix {
                lines.push(code_line(
                    12,
                    format!("// TODO: matriz no soportada: {matrix}"),
                ));
            }

            if !stage_env.is_empty() || !creds.is_empty() {
                lines.push(code_line(12, "environment {"));
                for (key, value) in &stage_env {
                    let rendered = if value.starts_with("credentials(") && value.ends_with(')') {
                        value.clone()
                    } else {
                        quote(value)
                    };
                    lines.push(code_line(16, format!("{} = {}", env_key(key), rendered)));
                }
                for cred in &creds {
                    if stage_env.keys().any(|key| env_key(key) == *cred) {
                        continue;
                    }
                    lines.push(code_line(
                        16,
                        format!("{} = credentials({})", cred, quote(cred)),
                    ));
                }
                lines.push(code_line(12, "}"));
            }

            if let Some(minutes) = job.timeout_minutes {
                lines.push(code_line(12, "options {"));
                lines.push(code_line(
                    16,
                    format!("timeout(time: {minutes}, unit: 'MINUTES')"),
                ));
                lines.push(code_line(12, "}"));
            }

            lines.push(code_line(12, "steps {"));
            lines.extend(body);
            lines.push(code_line(12, "}"));
            lines.push(code_line(8, "}"));

            stages.push(lines);
            stage_count += 1;
        }
    }

    if any_needs {
        warnings.push(
            "Los jobs se han ordenado según `needs`; Jenkins los ejecuta en secuencia (GitHub podía ejecutarlos en paralelo)."
                .to_string(),
        );
    }
    if any_runs_on {
        warnings.push(
            "`runs-on` se ignora: el pipeline usa `agent any` y se ejecuta en el propio Termux/Android."
                .to_string(),
        );
    }

    let mut content = String::new();
    content.push_str("// Jenkinsfile generado por `termux-cicd migrate`.\n");
    content.push_str("//\n// Origen:\n");
    for workflow in &workflows {
        content.push_str(&format!("//   .github/workflows/{}\n", workflow.file));
    }
    content.push_str("//\n");
    content.push_str(
        "// GitHub Actions y Jenkins no son equivalentes. Revisa los TODO y los avisos\n\
         // antes de confiar en este pipeline. Los cuerpos de `run` se escriben sin\n\
         // sangría para conservar su contenido literal (heredocs incluidos).\n",
    );
    if !warnings.is_empty() {
        content.push_str("//\n// Avisos de migración:\n");
        for warning in &warnings {
            for line in wrap_text(warning, 74) {
                content.push_str(&format!("//   {line}\n"));
            }
        }
    }
    content.push_str("\npipeline {\n");
    content.push_str("    agent any\n\n");
    content.push_str("    options {\n");
    content.push_str("        timestamps()\n");
    content.push_str("        disableConcurrentBuilds()\n");
    content.push_str("        buildDiscarder(logRotator(numToKeepStr: '10'))\n");
    content.push_str("    }\n\n");
    content.push_str("    environment {\n");
    content.push_str("        CI = 'true'\n");
    content.push_str("    }\n\n");
    content.push_str("    stages {\n");
    for stage in &stages {
        for line in stage {
            content.push_str(&format!("{}\n", line.render()));
        }
        content.push('\n');
    }
    content.push_str("    }\n\n");
    content.push_str("    post {\n        always {\n            cleanWs()\n        }\n    }\n");
    content.push_str("}\n");

    Ok(Generated {
        content,
        workflows: workflows
            .iter()
            .map(|workflow| workflow.file.clone())
            .collect(),
        warnings,
        stages: stage_count,
        supported_steps,
    })
}

fn render_step(
    step: &Step,
    base: usize,
    creds: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> (Vec<Line>, bool) {
    if let Some(condition) = &step.condition {
        push_unique(
            warnings,
            format!(
                "condición `if:` no portable, se ejecuta siempre: {}",
                one_line(condition)
            ),
        );
    }
    if step.continue_on_error {
        push_unique(
            warnings,
            "`continue-on-error` no se migra; usa `catchError` si lo necesitas.".to_string(),
        );
    }
    if let Some(minutes) = step.timeout_minutes {
        push_unique(
            warnings,
            format!(
                "`timeout-minutes: {minutes}` a nivel de paso no se migra; añade un `timeout`."
            ),
        );
    }

    let mut lines = Vec::new();
    let mut supported = false;
    if let Some(name) = &step.name {
        lines.push(code_line(base, format!("// {}", one_line(name))));
    }

    if let Some(uses) = &step.uses {
        let (use_lines, use_supported) = render_uses(step, uses, base, warnings);
        lines.extend(use_lines);
        supported |= use_supported;
    } else if let Some(script) = &step.run {
        if let Some(shell) = step.shell.as_deref() {
            let shell_lc = shell.to_ascii_lowercase();
            if shell_lc.contains("pwsh") || shell_lc.contains("powershell") {
                push_unique(
                    warnings,
                    format!("`shell: {shell}` no es portable; el paso se deja comentado."),
                );
                lines.push(code_line(
                    base,
                    "// TODO: migrar este script de PowerShell a sh/bash",
                ));
                for line in script.lines() {
                    lines.push(code_line(base, format!("//   {line}")));
                }
                return (wrap_step_env(step, lines, base, warnings), false);
            }
            if shell_lc.contains("cmd") || shell_lc.contains("python") {
                push_unique(
                    warnings,
                    format!("`shell: {shell}` no es portable; revisa el paso."),
                );
            }
        }

        let script = render_expressions(script, creds, warnings);
        lines.push(code_line(base, "sh '''"));
        for line in escape_script(&script).lines() {
            lines.push(Line::Raw(line.to_string()));
        }
        lines.push(code_line(base, "'''"));
        supported = true;
    } else {
        push_unique(
            warnings,
            "paso sin `run` ni `uses`; no se puede convertir.".to_string(),
        );
        lines.push(code_line(base, "// TODO: paso no reconocido"));
    }

    if let Some(dir) = &step.working_directory {
        lines = block(base, format!("dir({}) {{", quote(dir)), lines, "}");
    }

    (wrap_step_env(step, lines, base, warnings), supported)
}

fn wrap_step_env(
    step: &Step,
    lines: Vec<Line>,
    base: usize,
    warnings: &mut Vec<String>,
) -> Vec<Line> {
    if step.env.is_empty() {
        return lines;
    }

    let mut pairs = Vec::new();
    for (key, value) in &step.env {
        if value.contains("${{") {
            push_unique(
                warnings,
                format!(
                    "`env.{key}` a nivel de paso contiene una expresión; configúrala con withCredentials."
                ),
            );
        }
        pairs.push(quote(&format!("{}={}", env_key(key), value)));
    }
    block(
        base,
        format!("withEnv([{}]) {{", pairs.join(", ")),
        lines,
        "}",
    )
}

fn render_uses(
    step: &Step,
    uses: &str,
    base: usize,
    warnings: &mut Vec<String>,
) -> (Vec<Line>, bool) {
    let lower = uses.to_ascii_lowercase();
    let mut lines = Vec::new();

    if lower.starts_with("actions/checkout") {
        lines.push(code_line(
            base,
            "// actions/checkout: el checkout lo hace Jenkins desde el SCM",
        ));
        return (lines, false);
    }

    if lower.starts_with("actions/upload-artifact") {
        let path = step.with.get("path").map(|value| {
            value
                .split(['\n', '\r'])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        });
        match path {
            Some(path) if !path.is_empty() => {
                if let Some(name) = step.with.get("name") {
                    lines.push(code_line(base, format!("// artefacto: {}", one_line(name))));
                }
                let allow_empty = step
                    .with
                    .get("if-no-files-found")
                    .map(|value| value != "error")
                    .unwrap_or(true);
                lines.push(code_line(
                    base,
                    format!(
                        "archiveArtifacts artifacts: {}, allowEmptyArchive: {}",
                        quote(&path),
                        allow_empty
                    ),
                ));
                return (lines, true);
            }
            _ => {
                push_unique(
                    warnings,
                    "`upload-artifact` sin `path`; añade archiveArtifacts a mano.".to_string(),
                );
                lines.push(code_line(
                    base,
                    "// TODO: upload-artifact sin path; usa archiveArtifacts",
                ));
            }
        }
        return (lines, false);
    }

    let reason = if lower.starts_with("actions/download-artifact") {
        "requiere el plugin Copy Artifact o descargar desde el job anterior"
    } else if lower.starts_with("actions/setup-") {
        "instala el toolchain en Termux y elimina este paso"
    } else if lower.starts_with("actions/cache") {
        "Jenkins no comparte caché entre builds de la misma forma"
    } else if lower.starts_with("docker/") {
        "Docker no está disponible de forma nativa en Termux; revisa el paso"
    } else if lower.starts_with("actions/github-script") {
        "sustitúyelo por un script Groovy o `sh` con la API de GitHub"
    } else if uses.starts_with("./") {
        "acción local de GitHub; migra su lógica a `sh` o a un script del repo"
    } else {
        "acción del Marketplace sin equivalente directo"
    };

    push_unique(
        warnings,
        format!("`{uses}` no tiene equivalente automático: {reason}."),
    );
    lines.push(code_line(
        base,
        format!("// TODO: reemplazar {} ({reason})", one_line(uses)),
    ));
    (lines, false)
}

fn merged_env(
    workflow: &BTreeMap<String, String>,
    job: &BTreeMap<String, String>,
    creds: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    for (key, value) in workflow.iter().chain(job.iter()) {
        let value = value.trim();
        if let Some(secret) = single_secret(value) {
            let var = env_key(&secret);
            let warning = if var != secret {
                format!(
                    "secreto `{secret}` mapeado a la credencial `{var}`; crea esa credencial en Jenkins (Secret text)."
                )
            } else {
                format!(
                    "secreto `{secret}` mapeado a la credencial `{secret}`; créala en Jenkins (Secret text)."
                )
            };
            push_unique(warnings, warning);
            creds.insert(var.clone());
            merged.insert(key.clone(), format!("credentials('{var}')"));
        } else {
            if value.contains("${{") {
                push_unique(
                    warnings,
                    format!(
                        "`env.{key}` contiene una expresión no portable: {}",
                        one_line(value)
                    ),
                );
            }
            merged.insert(key.clone(), value.to_string());
        }
    }
    merged
}

fn single_secret(value: &str) -> Option<String> {
    let inner = value.strip_prefix("${{")?.strip_suffix("}}")?.trim();
    inner
        .strip_prefix("secrets.")
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// Sustituye expresiones `${{ ... }}` por su equivalente en Jenkins.
fn render_expressions(
    script: &str,
    creds: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut rest = script;

    while let Some(start) = rest.find("${{") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let expression = after[..end].trim();
        out.push_str(&rest[..start]);
        out.push_str(&map_expression(expression, creds, warnings));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn map_expression(
    expression: &str,
    creds: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> String {
    if let Some(name) = expression.strip_prefix("secrets.") {
        let var = env_key(name);
        let warning = if var != name {
            format!(
                "secreto `{name}` mapeado a la credencial `{var}`; crea esa credencial en Jenkins (Secret text)."
            )
        } else {
            format!(
                "secreto `{name}` mapeado a la credencial `{name}`; créala en Jenkins (Secret text)."
            )
        };
        push_unique(warnings, warning);
        creds.insert(var.clone());
        return format!("${{{var}}}");
    }

    match expression {
        "github.sha" => "${GIT_COMMIT}".to_string(),
        "github.ref_name" | "github.ref" => "${BRANCH_NAME}".to_string(),
        "github.workspace" => "${WORKSPACE}".to_string(),
        "github.repository" => {
            push_unique(
                warnings,
                "`github.repository` no está definido en Jenkins; exporta GITHUB_REPOSITORY o sustitúyelo."
                    .to_string(),
            );
            "${GITHUB_REPOSITORY}".to_string()
        }
        "runner.os" => "Linux".to_string(),
        _ => {
            if let Some(name) = expression.strip_prefix("env.") {
                return format!("${{{}}}", env_key(name));
            }
            push_unique(
                warnings,
                format!(
                    "expresión no portable: `${{{{ {} }}}}`; revisa el script.",
                    one_line(expression)
                ),
            );
            "${TERMUX_CICD_EXPR}".to_string()
        }
    }
}

fn order_jobs(jobs: &[Job]) -> (Vec<usize>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut indegree = vec![0usize; jobs.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); jobs.len()];
    let mut index = HashMap::new();
    for (position, job) in jobs.iter().enumerate() {
        index.insert(job.id.as_str(), position);
    }

    for (position, job) in jobs.iter().enumerate() {
        for need in &job.needs {
            match index.get(need.as_str()) {
                Some(&dependency) if dependency != position => {
                    dependents[dependency].push(position);
                    indegree[position] += 1;
                }
                Some(_) => {
                    warnings.push(format!(
                        "el job `{}` depende de sí mismo; se ignora `needs`.",
                        job.id
                    ));
                }
                None => {
                    warnings.push(format!(
                        "el job `{}` depende de `{need}`, que no existe; se ignora.",
                        job.id
                    ));
                }
            }
        }
    }

    let mut ready: Vec<usize> = (0..jobs.len()).filter(|&i| indegree[i] == 0).collect();
    ready.sort();
    let mut order = Vec::new();
    while !ready.is_empty() {
        let current = ready.remove(0);
        order.push(current);
        for &dependent in &dependents[current] {
            indegree[dependent] -= 1;
            if indegree[dependent] == 0 {
                ready.push(dependent);
            }
        }
        ready.sort();
    }

    if order.len() < jobs.len() {
        warnings.push(
            "hay dependencias circulares en `needs`; se usa el orden del archivo.".to_string(),
        );
        for position in 0..jobs.len() {
            if !order.contains(&position) {
                order.push(position);
            }
        }
    }

    (order, warnings)
}

fn matrix_summary(value: &Value) -> String {
    let map = match value.as_mapping() {
        Some(map) => map,
        None => return scalar(value).unwrap_or_else(|| "desconocida".to_string()),
    };
    let keys: Vec<String> = map
        .keys()
        .filter_map(scalar)
        .filter(|key| !key.starts_with("include") && !key.starts_with("exclude"))
        .collect();
    if keys.is_empty() {
        "con include/exclude".to_string()
    } else {
        format!("con {}", keys.join(", "))
    }
}

fn field<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.get(Value::String(key.to_string()))
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(Value::Mapping(map)) = value {
        for (key, value) in map {
            if let Some(key) = scalar(key) {
                out.insert(key, scalar(value).unwrap_or_else(|| format!("{value:?}")));
            }
        }
    }
    out
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Sequence(sequence)) => sequence.iter().filter_map(scalar).collect(),
        Some(value) => scalar(value).into_iter().collect(),
        None => Vec::new(),
    }
}

fn push_unique(warnings: &mut Vec<String>, warning: String) {
    if !warnings.contains(&warning) {
        warnings.push(warning);
    }
}

fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn env_key(raw: &str) -> String {
    let mut out = String::new();
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            out.push(character.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("VAR");
    }
    if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        out.insert(0, '_');
    }
    out
}

fn quote(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace(['\n', '\r'], " ");
    format!("'{escaped}'")
}

/// Escapa un script para un bloque Groovy `'''...'''` de una sola cadena.
fn escape_script(script: &str) -> String {
    script.replace('\\', "\\\\").replace("'''", "\\'\\'\\'")
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[derive(Debug, Clone)]
enum Line {
    Code(String),
    Raw(String),
}

impl Line {
    fn render(&self) -> &str {
        match self {
            Line::Code(line) | Line::Raw(line) => line,
        }
    }
}

fn code_line(indent: usize, text: impl AsRef<str>) -> Line {
    Line::Code(format!("{}{}", " ".repeat(indent), text.as_ref()))
}

fn indent(lines: Vec<Line>, extra: usize) -> Vec<Line> {
    if extra == 0 {
        return lines;
    }
    lines
        .into_iter()
        .map(|line| match line {
            Line::Code(text) => Line::Code(format!("{}{}", " ".repeat(extra), text)),
            raw => raw,
        })
        .collect()
}

fn block(base: usize, header: String, inner: Vec<Line>, footer: &str) -> Vec<Line> {
    let mut lines = vec![code_line(base, header)];
    lines.extend(indent(inner, 4));
    lines.push(code_line(base, footer));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn project(files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "termux-cicd-migrate-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let workflows = dir.join(".github").join("workflows");
        fs::create_dir_all(&workflows).unwrap();
        for (name, content) in files {
            fs::write(workflows.join(name), content).unwrap();
        }
        dir
    }

    fn convert(yaml: &str) -> Generated {
        let dir = project(&[("ci.yml", yaml)]);
        generate(&dir).unwrap().unwrap()
    }

    #[test]
    fn sin_workflows_devuelve_none() {
        let dir = std::env::temp_dir().join(format!(
            "termux-cicd-migrate-vacio-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        assert!(generate(&dir).unwrap().is_none());
    }

    #[test]
    fn convierte_run_y_omite_checkout() {
        let generated = convert(
            r#"
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Compilar
        run: cargo build --release
"#,
        );
        assert!(generated.usable());
        assert!(generated.content.contains("stage('build')"));
        assert!(generated.content.contains("sh '''"));
        assert!(generated.content.contains("cargo build --release"));
        assert!(!generated.content.contains("actions/checkout@v4"));
        assert!(!generated.content.contains("archiveArtifacts"));
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("on:")));
    }

    #[test]
    fn ordena_jobs_por_needs() {
        let generated = convert(
            r#"
on: push
jobs:
  deploy:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - run: echo deploy
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
"#,
        );
        let build = generated.content.find("stage('build')").unwrap();
        let deploy = generated.content.find("stage('deploy')").unwrap();
        assert!(build < deploy);
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("needs")));
    }

    #[test]
    fn mapea_env_y_working_directory() {
        let generated = convert(
            r#"
on: push
env:
  GLOBAL: "1"
jobs:
  build:
    runs-on: ubuntu-latest
    env:
      LOCAL: "2"
    steps:
      - run: npm ci
        working-directory: app
"#,
        );
        assert!(generated.content.contains("GLOBAL = '1'"));
        assert!(generated.content.contains("LOCAL = '2'"));
        assert!(generated.content.contains("dir('app') {"));
    }

    #[test]
    fn mapea_upload_artifact() {
        let generated = convert(
            r#"
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: make dist
      - uses: actions/upload-artifact@v4
        with:
          name: binarios
          path: |
            dist/*.zip
            dist/*.tar.gz
"#,
        );
        assert!(generated
            .content
            .contains("archiveArtifacts artifacts: 'dist/*.zip, dist/*.tar.gz'"));
        assert!(generated.content.contains("// artefacto: binarios"));
    }

    #[test]
    fn avisa_de_setup_node_sin_equivalente() {
        let generated = convert(
            r#"
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: node --version
"#,
        );
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("actions/setup-node@v4")));
        assert!(generated
            .content
            .contains("// TODO: reemplazar actions/setup-node@v4"));
    }

    #[test]
    fn solo_acciones_no_soportadas_no_es_usable() {
        let generated = convert(
            r#"
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version: 20
"#,
        );
        assert_eq!(generated.stages, 1);
        assert!(!generated.usable());
    }

    #[test]
    fn mapea_secretos_a_credenciales() {
        let generated = convert(
            r#"
on: push
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - run: |
          curl -H "Authorization: token ${{ secrets.GITHUB_TOKEN }}" https://api.github.com
"#,
        );
        assert!(generated
            .content
            .contains("GITHUB_TOKEN = credentials('GITHUB_TOKEN')"));
        assert!(generated.content.contains("${GITHUB_TOKEN}"));
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("credencial")));
    }

    #[test]
    fn mapea_secreto_en_env_sin_comillas() {
        let generated = convert(
            r#"
on: push
jobs:
  deploy:
    runs-on: ubuntu-latest
    env:
      TOKEN: ${{ secrets.API_TOKEN }}
    steps:
      - run: echo listo
"#,
        );
        assert!(generated
            .content
            .contains("TOKEN = credentials('API_TOKEN')"));
        assert!(!generated.content.contains("'credentials("));
    }

    #[test]
    fn avisa_de_matriz_y_expresiones_no_portables() {
        let generated = convert(
            r#"
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node: [18, 20]
    steps:
      - run: echo "${{ matrix.node }} ${{ github.event_name }}"
"#,
        );
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("matriz")));
        assert!(generated
            .warnings
            .iter()
            .any(|warning| warning.contains("github.event_name")));
        assert!(generated.content.contains("${TERMUX_CICD_EXPR}"));
    }

    #[test]
    fn escapa_comillas_y_barra_invertida() {
        let generated = convert(
            r#"
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: |
          printf 'hola\n' && echo "'''"
"#,
        );
        assert!(generated.content.contains("printf 'hola\\\\n'"));
        assert!(generated.content.contains("\\'\\'\\'"));
    }

    #[test]
    fn varios_workflows_prefijan_stage() {
        let dir = project(&[
            (
                "a.yml",
                "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo a\n",
            ),
            (
                "b.yml",
                "on: pull_request\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo b\n",
            ),
        ]);
        let generated = generate(&dir).unwrap().unwrap();
        assert!(generated.content.contains("stage('a / build')"));
        assert!(generated.content.contains("stage('b / build')"));
        assert_eq!(generated.stages, 2);
    }
}
