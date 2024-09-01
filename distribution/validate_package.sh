#!/bin/bash
if [[ -z "${APP_STORE_CONNECT_KEY_ID}" ]] || [[ -z "${APP_STORE_CONNECT_ISSUER_ID}" ]]; then
  echo "Missing APP_STORE_CONNECT_KEY_ID and/or APP_STORE_CONNECT_ISSUER_ID"
  exit 1
fi

# Validate the package
xcrun altool --validate-app -f target/release/bundle/osx/Shelv.pkg -t macos --apiKey "$APP_STORE_CONNECT_KEY_ID" --apiIssuer "$APP_STORE_CONNECT_ISSUER_ID"
