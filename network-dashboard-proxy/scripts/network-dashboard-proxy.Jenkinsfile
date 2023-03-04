@Library('concordium-pipelines') _
pipeline {
    agent any
    environment {
        ecr_repo_domain = '192549843005.dkr.ecr.eu-west-1.amazonaws.com'
    }
    stages {
        stage('ecr-login') {
            steps {
                ecrLogin(env.ecr_repo_domain, 'eu-west-1')
            }
        }
        stage('build') {
            environment {
                image_repo = "${ecr_repo_domain}/concordium/middleware"
                image_name = "${image_repo}:${image_tag}"
            }
            steps {
                sh '''\
                     docker build \
                        --build-arg build_image=rust:${rust_version}-buster\
                        --build-arg base_image=debian:buster\
                        --label rust_version="${rust_version}" \
                        -f network-dashboard-proxy/scripts/build.Dockerfile\
                        -t "${image_name}"\
                        --no-cache\
                        .
                    docker push "${image_name}"
                '''.stripIndent()
            }
        }
    }
}
