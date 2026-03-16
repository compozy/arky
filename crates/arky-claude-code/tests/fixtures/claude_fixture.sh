#!/bin/sh
set -eu

if [ "${1-}" = "--version" ]; then
  printf '%s\n' "${CLAUDE_FIXTURE_VERSION:-claude-fixture 1.0.0}"
  exit 0
fi

SESSION_ID="${CLAUDE_FIXTURE_SESSION_ID:-fixture-session}"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --session-id)
      SESSION_ID="$2"
      shift 2
      ;;
    --output-format|--model)
      shift 2
      ;;
    --print|--verbose)
      shift 1
      ;;
    *)
      shift 1
      ;;
  esac
done

if [ -n "${CLAUDE_FIXTURE_STDERR:-}" ]; then
  printf '%s\n' "$CLAUDE_FIXTURE_STDERR" >&2
fi

MODE="${CLAUDE_FIXTURE_MODE:-contract_basic}"
case "$MODE" in
  contract_basic)
    printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$SESSION_ID\"}"
    printf '%s\n' "{\"type\":\"assistant\",\"session_id\":\"$SESSION_ID\",\"parent_tool_use_id\":null,\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"done\"}]}}"
    printf '%s\n' "{\"type\":\"result\",\"subtype\":\"success\",\"stop_reason\":\"end_turn\",\"usage\":{\"input_tokens\":11,\"output_tokens\":5,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0},\"session_id\":\"$SESSION_ID\"}"
    ;;
  tool_cycle)
    printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$SESSION_ID\"}"
    printf '%s\n' "{\"type\":\"stream_event\",\"session_id\":\"$SESSION_ID\",\"event\":{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tool-1\",\"name\":\"search\",\"input\":{}}}}"
    printf '%s\n' "{\"type\":\"stream_event\",\"session_id\":\"$SESSION_ID\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\\\"docs\\\"}\"}}}"
    printf '%s\n' "{\"type\":\"stream_event\",\"session_id\":\"$SESSION_ID\",\"event\":{\"type\":\"content_block_stop\",\"index\":0}}"
    printf '%s\n' "{\"type\":\"user\",\"session_id\":\"$SESSION_ID\",\"parent_tool_use_id\":null,\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tool-1\",\"name\":\"search\",\"content\":[{\"type\":\"text\",\"text\":\"done\"}],\"is_error\":false}]}}"
    printf '%s\n' "{\"type\":\"assistant\",\"session_id\":\"$SESSION_ID\",\"parent_tool_use_id\":null,\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"after tool\"}]}}"
    printf '%s\n' "{\"type\":\"result\",\"subtype\":\"success\",\"stop_reason\":\"end_turn\",\"usage\":{\"input_tokens\":12,\"output_tokens\":8,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0},\"session_id\":\"$SESSION_ID\"}"
    ;;
  malformed)
    printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$SESSION_ID\"}"
    printf '%s\n' "{not json"
    ;;
  crash_after_first_event)
    printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$SESSION_ID\"}"
    printf '%s\n' 'fixture crashed' >&2
    exit 7
    ;;
  *)
    printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$SESSION_ID\"}"
    printf '%s\n' "{\"type\":\"assistant\",\"session_id\":\"$SESSION_ID\",\"parent_tool_use_id\":null,\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$MODE\"}]}}"
    printf '%s\n' "{\"type\":\"result\",\"subtype\":\"success\",\"stop_reason\":\"end_turn\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0},\"session_id\":\"$SESSION_ID\"}"
    ;;
esac
