#!/bin/bash
# usage: run.sh N "question"
N=$1; Q=$2; OUT=/Users/abdullahatrash/flowleap/wt-98c/acceptance/runs-3
cd /tmp/fl-accept || exit 1
date -u +%FT%TZ > $OUT/q$N.started
CLAUDE_CODE_DISABLE_CLAUDE_MDS=1 CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude -p "$Q" \
  --mcp-config /Users/abdullahatrash/flowleap/wt-98c/acceptance/mcp.json --strict-mcp-config \
  --disable-slash-commands --tools "ListMcpResourcesTool,ReadMcpResourceTool" \
  --allowedTools "mcp__flowleap__*" "ListMcpResourcesTool" "ReadMcpResourceTool" \
  --permission-mode dontAsk --output-format stream-json --verbose --max-turns 25 \
  --no-session-persistence > $OUT/q$N.json 2> $OUT/q$N.err
echo "exit=$?"
