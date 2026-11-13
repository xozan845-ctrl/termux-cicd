pipeline {
    agent any
    options {
        timestamps()
        disableConcurrentBuilds()
        buildDiscarder(logRotator(numToKeepStr: '10'))
    }
    stages {
        stage('Probar') {
            steps {
                sh 'go test ./...'
            }
        }
        stage('Compilar') {
            steps {
                sh 'go build ./...'
            }
        }
    }
    post {
        always {
            cleanWs()
        }
    }
}
