import jenkins.model.Jenkins
import jenkins.model.JenkinsLocationConfiguration
import hudson.security.HudsonPrivateSecurityRealm
import hudson.security.FullControlOnceLoggedInAuthorizationStrategy
import java.io.File

def instance = Jenkins.get()

// Crear el usuario admin solo la primera vez, con la contraseña generada.
if (!instance.isUseSecurity()) {
    def passwordFile = new File('{{PASSWORD_FILE}}')
    if (passwordFile.exists()) {
        def realm = new HudsonPrivateSecurityRealm(false)
        realm.createAccount('admin', passwordFile.text.trim())
        instance.setSecurityRealm(realm)
        def strategy = new FullControlOnceLoggedInAuthorizationStrategy()
        strategy.setAllowAnonymousRead(false)
        instance.setAuthorizationStrategy(strategy)
        instance.save()
        println 'termux-cicd: usuario admin creado'
    }
}

// Ajustes pensados para un móvil: pocos ejecutores.
instance.setNumExecutors({{EXECUTORS}})
instance.setSystemMessage('Jenkins personal en Termux (termux-cicd)')

// URL configurada en url.txt
def urlFile = new File('{{URL_FILE}}')
if (urlFile.exists()) {
    def location = JenkinsLocationConfiguration.get()
    location.setUrl(urlFile.text.trim())
    location.save()
}

instance.save()
