import jenkins.model.Jenkins
import jenkins.branch.BranchSource
import org.jenkinsci.plugins.github_branch_source.GitHubSCMSource
import org.jenkinsci.plugins.github_branch_source.BranchDiscoveryTrait
import org.jenkinsci.plugins.github_branch_source.OriginPullRequestDiscoveryTrait
import org.jenkinsci.plugins.workflow.multibranch.WorkflowMultiBranchProject
import org.jenkinsci.plugins.workflow.multibranch.WorkflowBranchProjectFactory
import com.cloudbees.hudson.plugins.folder.computed.PeriodicFolderTrigger
import com.cloudbees.plugins.credentials.CredentialsScope
import com.cloudbees.plugins.credentials.SystemCredentialsProvider
import com.cloudbees.plugins.credentials.domains.Domain
import com.cloudbees.plugins.credentials.impl.UsernamePasswordCredentialsImpl

def jobName = '{{JOB_NAME}}'
def owner = '{{OWNER}}'
def repo = '{{REPO}}'
def credentialId = '{{CREDENTIALS_ID}}'
def username = '{{USER}}'
def token = '{{TOKEN}}'
def trigger = '{{TRIGGER}}'
def scan = {{SCAN}}

if (token) {
    def store = SystemCredentialsProvider.getInstance().getStore()
    def existing = store.getCredentials(Domain.global()).find { it.id == credentialId }
    def credential = new UsernamePasswordCredentialsImpl(
        CredentialsScope.GLOBAL,
        credentialId,
        'GitHub PAT (termux-cicd)',
        username,
        token
    )
    if (existing != null) {
        store.updateCredentials(Domain.global(), existing, credential)
    } else {
        store.addCredentials(Domain.global(), credential)
    }
}

def instance = Jenkins.get()
def project = instance.getItem(jobName)
if (project == null) {
    project = instance.createProject(WorkflowMultiBranchProject, jobName)
} else if (!(project instanceof WorkflowMultiBranchProject)) {
    error "ya existe un job llamado '${jobName}' y no es un Multibranch Pipeline"
}

def source = new GitHubSCMSource(owner, repo)
source.setCredentialsId(credentialId ?: null)
source.setTraits([
    new BranchDiscoveryTrait(1),
    new OriginPullRequestDiscoveryTrait(1)
])
project.setSourcesList([new BranchSource(source)])

def factory = new WorkflowBranchProjectFactory()
factory.setScriptPath('Jenkinsfile')
project.setProjectFactory(factory)

project.getTriggers().values().toList().each { project.removeTrigger(it) }
if (trigger) {
    project.addTrigger(new PeriodicFolderTrigger(trigger))
}

project.save()
if (scan) {
    project.scheduleBuild2(0)
}
println "OK ${jobName}"
