import jenkins.model.Jenkins
import org.jenkinsci.plugins.workflow.multibranch.WorkflowMultiBranchProject
import org.jenkinsci.plugins.github_branch_source.GitHubSCMSource

def projects = Jenkins.get().getAllItems(WorkflowMultiBranchProject).collect().sort { it.fullName }
if (projects.isEmpty()) {
    println 'No hay Multibranch Pipeline configurados.'
} else {
    println String.format('%-28s %-42s %-6s %s', 'JOB', 'REPO', 'RAMAS', 'ULTIMO SCAN')
    projects.each { p ->
        def src = p.getSCMSources().find { it instanceof GitHubSCMSource }
        def repo = src ? "${src.repoOwner}/${src.repository}" : '(sin GitHub)'
        def computation = p.getComputation()
        def result = computation != null ? computation.getResult() : null
        println String.format('%-28s %-42s %-6s %s', p.fullName, repo, p.getItems().size(), result ?: 'sin scan')
    }
}
