#!/usr/bin/env python3
"""发布 workflow 不变式检查（判定单一来源、触发面、幂等入口）。

用法：python3 scripts/check-workflows.py
    - 任何环境下都跑文本不变式；失败 exit 1。
    - 有 pyyaml 时额外解析每个 workflow 的 YAML 语法，缺失则打印 SKIP 并只跑不变式。
    - 本地无 pyyaml 可用 uv 跑：
      UV_CACHE_DIR=$PWD/.tmp-test/uv-cache uv run --no-project --with pyyaml python scripts/check-workflows.py
"""
from __future__ import annotations

import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WF_DIR = os.path.join(ROOT, ".github", "workflows")

failures: list[str] = []


def read(name: str) -> str:
    with open(os.path.join(WF_DIR, name), encoding="utf-8") as fh:
        return fh.read()


def on_block(text: str) -> str:
    """取顶层 `on:` 块（缩进行，直到下一个顶层键）。"""
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if re.match(r"^on:\s*$", line):
            start = i + 1
            break
    if start is None:
        return ""
    out = []
    for line in lines[start:]:
        if line.strip() and not line[0].isspace() and not line.startswith("#"):
            break
        out.append(line)
    return "\n".join(out)


def check(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'PASS' if ok else 'FAIL'} {name}" + (f" — {detail}" if not ok else ""))
    if not ok:
        failures.append(name)


# ── 不变式：发布判定只有一处实现（scripts/release-gate.sh）─────────────
for wf in ("publish-crates-io.yml", "publish-pypi.yml"):
    text = read(wf)
    check(f"{wf}: gate 调用 scripts/release-gate.sh", "scripts/release-gate.sh" in text)
    check(
        f"{wf}: 无内联 stable/预发布判定",
        not re.search(r"case \"?\$VERSION\"?|-\*-alpha|-alpha\*|is_stable=", text),
        "判定逻辑必须只在 scripts/release-gate.sh（#506）",
    )

# ── YAML 语法（可选：需要 pyyaml）─────────────────────────────────────
try:
    import yaml  # type: ignore
except ImportError:
    print(
        "SKIP YAML 语法解析：pyyaml 未安装"
        "（CI 会装；本地：uv run --no-project --with pyyaml python scripts/check-workflows.py）"
    )
else:
    for path in sorted(glob.glob(os.path.join(WF_DIR, "*.yml"))):
        name = os.path.basename(path)
        try:
            with open(path, encoding="utf-8") as fh:
                yaml.safe_load(fh)
        except Exception as exc:  # noqa: BLE001 - 语法错误统一报告
            check(f"YAML 语法 {name}", False, str(exc))
        else:
            check(f"YAML 语法 {name}", True)

print()
if failures:
    print(f"workflow 检查失败：{len(failures)} 项")
    sys.exit(1)
print("workflow 检查全部通过")
