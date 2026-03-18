#!/bin/bash

set -o errexit -o nounset -o pipefail
rm -rf apps/
blacklist=("app.grapheneos.apps" "app.attestation.auditor" "app.grapheneos.info")

for pkg in $(jq -r '.packages | keys[]' metadata.sjson); do
  if [[ " ${blacklist[*]} " =~ " ${pkg} " ]]; then
    continue
  fi

  mkdir -p apps/packages/${pkg}
  if [ ! -f "apps/packages/${pkg}/common-props.toml" ]; then
    curl -L -o "apps/packages/${pkg}/common-props.toml" \
      "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/common-props.toml"
  fi
  if curl -sfLo/dev/null -r0-0 "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/${pkg}/icon.webp"; then
    curl -L -o "apps/packages/${pkg}/icon.webp" \
      "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/icon.webp"
  fi

  for version in $(jq -r ".packages[\"$pkg\"].variants | keys[]" metadata.sjson); do
    mkdir apps/packages/${pkg}/${version}

    # Download channel.toml for this variant
    if curl -sfLo/dev/null -r0-0 --fail "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/$version/channel.toml"; then
      curl -L -o "apps/packages/${pkg}/${version}/channel.toml" \
        "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/$version/channel.toml"
    fi

    # props.toml
    if curl -sfLo/dev/null -r0-0 --fail "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/$version/props.toml"; then
      curl -L -o "apps/packages/${pkg}/${version}/props.toml" \
        "https://raw.githubusercontent.com/GrapheneOS/apps.grapheneos.org/refs/heads/main/apps/packages/$pkg/$version/props.toml"
    fi

    # Download all APKs and idsig files
    for apkFile in $(jq -r ".packages[\"$pkg\"].variants[\"$version\"].apks[]" metadata.sjson); do
      urlBase="https://apps.grapheneos.org/packages/$pkg/$version"
      curl -L -o "apps/packages/${pkg}/${version}/${apkFile}.gz" "$urlBase/$apkFile.gz"
      curl -L -o "apps/packages/${pkg}/${version}/${apkFile}.br" "$urlBase/$apkFile.br"
      cd apps/packages/${pkg}/${version}
      gunzip -k ${apkFile}.gz
      touch -r "${apkFile}" "${apkFile}.gz" "${apkFile}.br"
      cd ../../../..
      if [[ $(jq -r ".packages[\"$pkg\"].variants[\"$version\"].hasV4Signatures" metadata.sjson) == true ]]; then
        curl -L -o "apps/packages/${pkg}/${version}/${apkFile}.idsig" "$urlBase/$apkFile.idsig"
      fi
    done
  done
done

./generate.py
./process-static