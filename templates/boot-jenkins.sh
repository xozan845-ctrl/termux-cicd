#!/data/data/com.termux/files/usr/bin/sh
# Arranque automático al encender el móvil (requiere la app Termux:Boot)
export PATH="{{PREFIX}}/bin:$PATH"
termux-wake-lock
sleep 5
export SVDIR="{{PREFIX}}/var/service"
export LOGDIR="{{PREFIX}}/var/log"
service-daemon start >/dev/null 2>&1 &
sleep 3
sv-enable jenkins >/dev/null 2>&1 || sv start jenkins >/dev/null 2>&1
