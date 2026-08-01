"""FeatherView 示例 Python 文件"""

import json


def load_config(path: str) -> dict:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


if __name__ == "__main__":
    data = {"name": "featherview", "version": "0.1.0"}
    print(json.dumps(data, ensure_ascii=False))
