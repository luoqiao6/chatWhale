#!/usr/bin/env bash
# 最小 MCP stdio server（JSON-RPC 2.0 over NDJSON）：initialize / initialized / tools/list / tools/call
while IFS= read -r line; do
  [ -z "$line" ] && continue
  method=$(printf '%s' "$line" | sed -n 's/.*"method":"\([^"]*\)".*/\1/p')
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$method" in
    initialize)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"fake","version":"1.0.0"}}}\n' "$id"
      ;;
    notifications/initialized)
      ;;
    shutdown)
      exit 0
      ;;
    tools/list)
      printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"echo","description":"echo text","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}]}}\n' "$id"
      ;;
    tools/call)
      text=$(printf '%s' "$line" | sed -n 's/.*"arguments":{[^}]*"text":"\([^"]*\)".*/\1/p')
      printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"echo:%s"}]}}\n' "$id" "$text"
      ;;
    *)
      printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"method not found"}}\n' "$id"
      ;;
  esac
done
