#!/usr/bin/env python3
"""Send one Jev (TypeSafe System One) decisions request through OpenRouter.

Usage:
    python3 scripts/jev.py REQUEST.json [-o RESPONSE.json] [--max-tokens N] [--dry-run]

REQUEST.json holds {"state": ..., "questions": {...}}; "model" defaults to typesafe/jev-1.14.
The API key comes from OPENROUTER_API_KEY, or from .env.local at the repository root. It is
never printed. Jev returns typed answers (noul, choice, score), not prose.
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

ENDPOINT = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.14"
# Jev 1.14 default per-request context is 64k tokens; stay under it with margin.
DEFAULT_MAX_TOKENS = 56_000
USD_PER_INPUT_TOKEN = 0.042 / 1_000_000
ROOT = Path(__file__).resolve().parent.parent


def api_key() -> str:
    key = os.environ.get("OPENROUTER_API_KEY")
    env = ROOT / ".env.local"
    if not key and env.exists():
        for line in env.read_text().splitlines():
            name, _, value = line.partition("=")
            if name.strip() == "OPENROUTER_API_KEY":
                key = value.strip().strip("'\"")
    if not key:
        sys.exit("jev: OPENROUTER_API_KEY is not set and not found in .env.local")
    return key


def estimate_tokens(request: dict) -> int:
    """A conservative estimate. Measured usage runs near 1.7 characters per token, plus per-question overhead."""
    return len(json.dumps(request)) * 2 // 3 + 40 * len(request.get("questions", {})) + 100


def decide(request: dict, max_tokens: int = DEFAULT_MAX_TOKENS, timeout: float = 120) -> dict:
    """Post a decisions request and return the parsed response."""
    request = {"model": MODEL, **request}
    tokens = estimate_tokens(request)
    if tokens > max_tokens:
        raise ValueError(f"request is about {tokens} tokens, over the {max_tokens} limit; split it")
    body = json.dumps(request).encode()
    http = urllib.request.Request(
        ENDPOINT,
        data=body,
        headers={
            "Authorization": f"Bearer {api_key()}",
            "Content-Type": "application/json",
            "X-Title": "blah design review",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(http, timeout=timeout) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode(errors="replace")[:2000]
        raise RuntimeError(f"Jev request failed: HTTP {error.code}: {detail}") from None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("request", type=Path)
    parser.add_argument("-o", "--output", type=Path)
    parser.add_argument("--max-tokens", type=int, default=DEFAULT_MAX_TOKENS)
    parser.add_argument("--dry-run", action="store_true", help="check size and cost without sending")
    args = parser.parse_args()
    request = json.loads(args.request.read_text())
    tokens = estimate_tokens({"model": MODEL, **request})
    print(
        f"jev: {len(request.get('questions', {}))} questions, ~{tokens} input tokens, "
        f"~${tokens * USD_PER_INPUT_TOKEN:.6f}",
        file=sys.stderr,
    )
    if args.dry_run:
        return
    result = decide(request, args.max_tokens)
    usage = result.get("usage", {})
    print(f"jev: {result.get('model')} used {usage.get('input_tokens')} tokens, ${usage.get('cost', 0):.6f}", file=sys.stderr)
    text = json.dumps(result, indent=2)
    if args.output:
        args.output.write_text(text + "\n")
    else:
        print(text)


if __name__ == "__main__":
    main()
