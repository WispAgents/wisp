#!/usr/bin/env bash
# The folder GitHub Pages publishes, and nothing else: the pages scripts/site.mjs generates (with sitemap.xml,
# robots.txt and llms.txt), the public media in docs/assets, the root files the site serves, and the docs site
# (apps/docs, built first: npm run build --workspace apps/docs) under /docs. docs/ itself also holds the plan, ADRs,
# QA notes and research: those are never published. The Pages workflow uploads this folder.
#
#   bash scripts/stage-site.sh [out]     default: _site
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-_site}"
rm -rf "$out"
mkdir -p "$out"
node scripts/site.mjs "$out"
cp -R docs/assets "$out/assets"
# served at the root: the installer, the favicon, IndexNow's key file, and the domain if one is set
for f in install.sh favicon.ico c74d7dc2f8b011de1d62484d8507a500.txt CNAME .nojekyll; do
  if [ -f "docs/$f" ]; then cp "docs/$f" "$out/$f"; fi
done
[ -f apps/docs/dist/index.html ] || { echo "stage-site: build the docs first (npm run build --workspace apps/docs)" >&2; exit 1; }
cp -R apps/docs/dist "$out/docs"
# no notes leak: outside /docs the site has no markdown (the docs publish each page's own markdown for agents, llms.txt),
# and none of docs/'s internal folders or notes is here
leaked=$(find "$out" -path "$out/docs" -prune -o \( -name '*.md' -o -name '*.mdx' \) -print | head -5)
for d in adr qa research ux design site; do [ -e "$out/$d" ] && leaked="$leaked $out/$d"; done
if [ -n "$leaked" ]; then
  echo "stage-site: internal files in the site:$leaked" >&2
  exit 1
fi
echo "stage-site: $(find "$out" -type f | wc -l | tr -d ' ') files → $out"
