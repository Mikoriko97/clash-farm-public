"""Прямий клієнт сокета аддона MCP for Blender (localhost:9876), коли MCP-інструменти
недоступні в сесії. Той самий протокол, що й у blender-mcp: один JSON-запит,
одна JSON-відповідь.

    python client/tools/blender_rpc.py scene                 # get_scene_info
    python client/tools/blender_rpc.py run  script.py        # execute_code з файлу; результат — змінна `result`
    python client/tools/blender_rpc.py shot out.png          # скриншот вʼюпорту
"""

import json
import socket
import sys
from pathlib import Path

HOST, PORT = "localhost", 9876


def call(command: str, params: dict | None = None, timeout: float = 600.0) -> dict:
    sock = socket.create_connection((HOST, PORT), timeout=timeout)
    sock.sendall(json.dumps({"type": command, "params": params or {}}).encode())
    buf = b""
    while True:
        chunk = sock.recv(1 << 16)
        if not chunk:
            break
        buf += chunk
        try:
            data = json.loads(buf.decode())
            break
        except ValueError:
            continue
    sock.close()
    return data


def run_code(code: str) -> dict:
    return call("execute_code", {"code": code})


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    cmd = argv[0]
    if cmd == "scene":
        out = call("get_scene_info")
    elif cmd == "run":
        out = run_code(Path(argv[1]).read_text(encoding="utf-8"))
    elif cmd == "shot":
        out = call("get_viewport_screenshot", {"filepath": str(Path(argv[1]).resolve()), "max_size": 1280})
    else:
        print("unknown command", cmd)
        return 2
    text = json.dumps(out, ensure_ascii=False, indent=1)
    print(text[:4000])
    return 0 if out.get("status") == "success" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
