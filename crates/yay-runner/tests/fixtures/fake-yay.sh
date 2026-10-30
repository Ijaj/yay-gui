#!/bin/sh
# Fake yay for tests. Usage: fake-yay.sh [exit-code] ; env FAKE_YAY_PASSWORD = expected password.
# Prompts for a sudo password (no echo), then asks to proceed, then exits.
echo ":: Synchronizing package databases..."
stty -echo 2>/dev/null
printf '[sudo] password for tester: '
IFS= read -r pw
stty echo 2>/dev/null
echo
if [ "$pw" != "${FAKE_YAY_PASSWORD:-hunter2}" ]; then
  echo "Sorry, try again."
  exit 3
fi
echo "resolving dependencies..."
printf ':: Proceed with installation? [Y/n] '
IFS= read -r ans
echo
case "$ans" in
  y|Y|"") echo "installing fakepkg..."; exit "${1:-0}" ;;
  *) echo "aborted"; exit 1 ;;
esac
