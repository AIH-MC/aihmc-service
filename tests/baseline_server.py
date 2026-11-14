"""Real loopback HTTP server used by the migration smoke test."""

import sys

from tests.baseline_support import main
import uvicorn

if __name__ == "__main__":
    uvicorn.run(main.app, host="127.0.0.1", port=int(sys.argv[1]))
