#!/data/data/com.termux/files/usr/bin/sh
exec 2>&1

export HOME="{{HOME}}"
export PREFIX="{{PREFIX}}"
export JENKINS_HOME="{{JENKINS_HOME}}"
export JAVA_HOME="$PREFIX/lib/jvm/java-21-openjdk"
export PATH="$JAVA_HOME/bin:$PREFIX/bin:$PATH"
export FONTCONFIG_PATH="$PREFIX/etc/fonts"
export FONTCONFIG_FILE="$PREFIX/etc/fonts/fonts.conf"

# Mantener la CPU activa mientras Jenkins corre
termux-wake-lock 2>/dev/null || true

exec java -Xms256m -Xmx{{HEAP_MB}}m -XX:MaxMetaspaceSize={{META_MB}}m \
  -Djava.awt.headless=true -Dfile.encoding=UTF-8 \
  -Djenkins.install.runSetupWizard=false \
  -jar "{{WAR}}" \
  --httpPort={{PORT}} --httpListenAddress=0.0.0.0
