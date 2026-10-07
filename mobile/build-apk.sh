#!/usr/bin/env bash
# Signed release APK of the Solana dApp Store app (ADR-042): the Solana Mobile web shell (a WebView with native wallet
# intents for Mobile Wallet Adapter) around the game's URL. The keystore and its passwords never enter the repo.
#
#   CF_APK_KEYSTORE=/secure/place/clashfarm.keystore \
#   SOLANA_MOBILE_KEYSTORE_PASSWORD=*** SOLANA_MOBILE_KEY_PASSWORD=*** \
#   [CF_APK_KEY_ALIAS=clashfarm] bash mobile/build-apk.sh
#
# App id, URL and version live in mobile/gradle.properties (SOLANA_MOBILE_*): bump SOLANA_MOBILE_VERSION_CODE for every
# release; the dApp Store refuses an update with the same code or a different signing key.
# JAVA_HOME (JDK 17+) and ANDROID_HOME (platform 37, build-tools 37, platform-tools) default to ~/dev-tools (RUNBOOK).
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
: "${CF_APK_KEYSTORE:?set CF_APK_KEYSTORE to the signing keystore (kept outside the repo, with a backup)}"
: "${SOLANA_MOBILE_KEYSTORE_PASSWORD:?set SOLANA_MOBILE_KEYSTORE_PASSWORD (environment only)}"
: "${SOLANA_MOBILE_KEY_PASSWORD:?set SOLANA_MOBILE_KEY_PASSWORD (environment only)}"
ALIAS="${CF_APK_KEY_ALIAS:-clashfarm}"
[ -f "$CF_APK_KEYSTORE" ] || { echo "no keystore at $CF_APK_KEYSTORE" >&2; exit 1; }
case "$(cd "$(dirname "$CF_APK_KEYSTORE")" && pwd)" in
  "$(cd "$DIR/.." && pwd)"*) git -C "$DIR" check-ignore -q "$CF_APK_KEYSTORE" || { echo "the keystore is inside the repo and not ignored" >&2; exit 1; } ;;
esac

TOOLS="${CF_DEV_TOOLS:-$HOME/dev-tools}"
export JAVA_HOME="${JAVA_HOME:-$TOOLS/jdk-17.0.20.1+1}"
export ANDROID_HOME="${ANDROID_HOME:-$TOOLS/android-sdk}"
export PATH="$JAVA_HOME/bin:$PATH"

# Gradle directly, the way `solana-mobile webshell build` does it (assembleRelease with the keystore as -P and the
# passwords from the environment), so no unlocked npm tree ever runs next to the release key. Every dependency is
# checked against gradle/verification-metadata.xml (sha256).
KEYSTORE="$CF_APK_KEYSTORE"
command -v cygpath > /dev/null && KEYSTORE="$(cygpath -w "$KEYSTORE")"
(cd "$DIR" && ./gradlew --no-daemon assembleRelease "-PSOLANA_MOBILE_KEYSTORE_PATH=$KEYSTORE" "-PSOLANA_MOBILE_KEYSTORE_ALIAS=$ALIAS")

APK="$DIR/app/build/outputs/apk/release/app-release.apk"
[ -f "$APK" ] || { echo "no APK at $APK" >&2; exit 1; }
APKSIGNER="$(ls -d "$ANDROID_HOME"/build-tools/*/ | sort -V | tail -1)apksigner.bat"
[ -f "$APKSIGNER" ] || APKSIGNER="${APKSIGNER%.bat}"
# The signing certificate (public): the same SHA-256 on every release, or the dApp Store refuses the update.
"$APKSIGNER" verify --print-certs "$APK" | grep -E "certificate (DN|SHA-256 digest)" | sort -u
echo "APK: $APK"
sha256sum "$APK"
