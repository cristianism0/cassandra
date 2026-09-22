#!/bin/sh
# This script is only to build the packages to release in different formats.
set -e

TAG="$1"

if [ -z "$TAG" ]; then
  echo "Erro: Forneça a tag/versão como argumento. Ex: $0 v1.0.0"
  exit 1
fi

cargo build --release

rm -rf dist/stage
mkdir -p dist/stage

cp target/release/cassandra cap.sh LICENSE README.md dist/stage/

nfpm pkg --config nfpm.yaml --packager deb --target dist/
nfpm pkg --config nfpm.yaml --packager rpm --target dist/
nfpm pkg --config nfpm.yaml --packager archlinux --target dist/

tar_name="cassandra_${TAG}_Linux_x86_64.tar.gz"

tar -czf "dist/${tar_name}" -C dist/stage .

cd dist/ && sha256sum cassandra* > SHA256SUM