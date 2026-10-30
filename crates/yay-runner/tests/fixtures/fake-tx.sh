#!/bin/sh
# Fake yay for transaction tests: prints its arguments, then behaves according
# to the last argument: "fail" exits 4, "hang" sleeps, "ask" asks [Y/n].
echo "fake-yay $*"
for last in "$@"; do :; done
case "$last" in
  fail) echo "error: target not found: fail"; exit 4 ;;
  hang) echo "building..."; sleep 30; exit 0 ;;
  ask)
    printf ':: Proceed with installation? [Y/n] '
    IFS= read -r ans
    [ "$ans" = "y" ] || exit 1 ;;
esac
echo "done $last"
