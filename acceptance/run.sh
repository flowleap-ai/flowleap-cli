#!/bin/bash
# usage: [MCP_CONFIG=...] [RUNS_DIR=...] [FLOWLEAP_GATE_TOKEN_FILE=...] run.sh N "question"
# MCP_CONFIG defaults to the stdio bridge (acceptance/mcp.json). For the hosted
# server, set MCP_CONFIG=acceptance/mcp-hosted.json and FLOWLEAP_GATE_TOKEN_FILE
# to a file that holds a personal token. The token is read at runtime into
# FLOWLEAP_GATE_TOKEN, which Claude Code expands in the Authorization header.
# Never put the token in a file under the repo.
N=$1; Q=$2
HERE=$(cd "$(dirname "$0")" && pwd)
MCP_CONFIG=${MCP_CONFIG:-$HERE/mcp.json}
RUNS_DIR=${RUNS_DIR:-$HERE/runs-3}
case $MCP_CONFIG in /*) ;; *) MCP_CONFIG=$PWD/$MCP_CONFIG ;; esac
case $RUNS_DIR in /*) ;; *) RUNS_DIR=$PWD/$RUNS_DIR ;; esac
if [ -n "$FLOWLEAP_GATE_TOKEN_FILE" ]; then
  FLOWLEAP_GATE_TOKEN=$(tr -d '\n\r ' < "$FLOWLEAP_GATE_TOKEN_FILE") || exit 1
  export FLOWLEAP_GATE_TOKEN
fi
mkdir -p "$RUNS_DIR" /tmp/fl-accept
cd /tmp/fl-accept || exit 1
date -u +%FT%TZ > "$RUNS_DIR/q$N.started"
CLAUDE_CODE_DISABLE_CLAUDE_MDS=1 CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude -p "$Q" \
  --mcp-config "$MCP_CONFIG" --strict-mcp-config \
  --disable-slash-commands --tools "ListMcpResourcesTool,ReadMcpResourceTool" \
  --allowedTools "mcp__flowleap__*" "ListMcpResourcesTool" "ReadMcpResourceTool" \
  --permission-mode dontAsk --output-format stream-json --verbose --max-turns 25 \
  --no-session-persistence > "$RUNS_DIR/q$N.json" 2> "$RUNS_DIR/q$N.err"
echo "exit=$?"
