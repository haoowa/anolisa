#!/usr/bin/env python3
"""Exercise the real CLI writer and standalone authenticated query process."""

import json
import os
from pathlib import Path
import secrets
import socket
import sqlite3
import subprocess
import tempfile
import time
from typing import Any
import unittest
import urllib.error
import urllib.request

BINARY = Path(__file__).resolve().parents[1] / "target/debug/tokenless"


class StatisticsHttpTest(unittest.TestCase):
    def test_cli_writer_query_after_exit_and_read_only_failures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            environment = {**os.environ, "TOKENLESS_DATA_DIR": temporary,
                           "TOKENLESS_STATS_ENABLED": "1", "TOKENLESS_SLS_ENABLED": "0",
                           "TOKENLESS_COMPRESSION_ENABLED": "1"}
            original = json.dumps({"output": "repeated fixture output " * 300})
            subprocess.run([str(BINARY), "compress-response", "--agent-id", "fixture",
                            "--session-id", "fixture-session", "--tool-use-id", "fixture-tool",
                            "--no-stash", "--truncate-strings-at", "64"], input=original,
                           text=True, env=environment, capture_output=True, check=True)
            connection = sqlite3.connect(directory / "stats.db")
            stored = connection.execute("SELECT id,timestamp,before_tokens,after_tokens,before_text,after_text FROM stats").fetchone()
            self.assertIsNotNone(stored)
            connection.close()
            token = secrets.token_urlsafe(32)
            credential = directory / "credential"
            credential.write_text(token + "\n")
            credential.chmod(0o600)
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 0))
                port = listener.getsockname()[1]
            process = subprocess.Popen([str(BINARY), "stats", "serve", "--data-dir", temporary,
                                        "--credential-file", str(credential), "--listen", f"127.0.0.1:{port}"],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                base = f"http://127.0.0.1:{port}/v1/stats/records"
                for _ in range(100):
                    if process.poll() is not None:
                        self.fail(process.stderr.read().decode())
                    try:
                        urllib.request.urlopen(base, timeout=0.2)
                    except urllib.error.HTTPError as error:
                        self.assertEqual(error.code, 401)
                        break
                    except urllib.error.URLError:
                        time.sleep(0.05)
                else:
                    self.fail("query service did not start")

                def query(suffix: str) -> dict[str, Any]:
                    request = urllib.request.Request(base + suffix, headers={"Authorization": f"Bearer {token}"})
                    with urllib.request.urlopen(request, timeout=5) as response:
                        self.assertEqual(response.headers["Cache-Control"], "no-store")
                        return json.load(response)

                page = query("?session_id=fixture-session&tool_use_id=fixture-tool")
                self.assertEqual(len(page["records"]), 1)
                record = page["records"][0]
                self.assertEqual(record["id"], stored[0])
                self.assertEqual(record["timestamp"], stored[1])
                self.assertEqual(record["before_tokens"], stored[2])
                self.assertEqual(record["after_tokens"], stored[3])
                self.assertEqual(record["mode"], "active")
                self.assertFalse(record["content_included"])
                detail = query(f"/{stored[0]}?include_content=true")["record"]
                self.assertEqual(detail["before_text"], stored[4])
                self.assertEqual(detail["after_text"], stored[5])
                self.assertTrue(detail["content_included"])
                (directory / "stats.db").rename(directory / "original.db")
                with self.assertRaises(urllib.error.HTTPError) as failure:
                    query("")
                self.assertEqual(failure.exception.code, 503)
                self.assertFalse((directory / "stats.db").exists())
            finally:
                process.terminate()
                process.wait(timeout=10)
                process.stderr.close()


if __name__ == "__main__":
    unittest.main()
