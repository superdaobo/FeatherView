#!/usr/bin/env python3
"""FeatherView nightly 测试夹具生成脚本。

用法：
    python tests/fixtures/generate-fixtures.py [--large-mb 2]

在脚本所在目录（tests/fixtures/）生成：
- sample.pdf           最小合法 PDF（1 页空白，%PDF-1.4 + xref 完整）
- sample.csv           UTF-8（含 BOM）中文 CSV（Excel 兼容）
- sample-chinese.zip   含中文文件名与嵌套中文目录的 ZIP
- large-sample.txt     大文本（默认 2MB，UTF-8 中英混合段落，可调）

生成物为二进制/大文件，建议：
- sample.pdf / sample-chinese.zip 体积小，可提交入库
- large-sample.txt（2MB）建议加入 .gitignore 或在 CI/测试前临时生成
"""
import io
import os
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))


def build_pdf() -> bytes:
    """构造最小合法 PDF：Catalog -> Pages -> Page(空白) -> Contents(空流)。"""
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",                                    # 1
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",                            # 2
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R >>",  # 3
    ]
    stream = b"q Q"
    objects.append(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")  # 4

    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"

    xref_pos = len(out)
    out += b"xref\n0 %d\n" % (len(objects) + 1)
    out += b"0000000000 65535 f \n"
    for off in offsets:
        out += b"%010d 00000 n \n" % off
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\n" % (len(objects) + 1)
    out += b"startxref\n%d\n%%%%EOF\n" % xref_pos
    return bytes(out)


def build_csv() -> bytes:
    lines = [
        "姓名,部门,工号,月薪(元)",
        "张三,研发部,E1001,18500",
        "李四,产品部,E1002,17200",
        "王五,测试部,E1003,16300",
        "赵六,运维部,E1004,15800",
    ]
    text = "\r\n".join(lines) + "\r\n"
    # UTF-8 with BOM（兼容 Excel 直接打开）
    return b"\xef\xbb\xbf" + text.encode("utf-8")


def build_zip() -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("中文文件名-说明.txt",
                    "这是中文文件名的文本文件。\nFeatherView ZIP fixture。\n")
        zf.writestr("nested/子目录/深层文件.md",
                    "# 嵌套中文路径\n\n用于测试 ZIP 中文路径展示与安全解压。\n")
        zf.writestr("readme.txt",
                    "FeatherView sample zip (with Chinese filenames).\n")
    return buf.getvalue()


def build_large_text(mb: int) -> bytes:
    para_cn = "览匣 FeatherView 夜间测试文本。这是一段用于大文件读取与虚拟滚动测试的中文段落。\n"
    para_en = "The quick brown fox jumps over the lazy dog. FeatherView large file fixture line.\n"
    target = mb * 1024 * 1024
    out = bytearray()
    i = 0
    while len(out) < target:
        block = (para_cn if i % 2 == 0 else para_en).encode("utf-8")
        if len(out) + len(block) > target:
            break
        out += block
        i += 1
    return bytes(out)


def main() -> None:
    large_mb = 2
    for arg in sys.argv[1:]:
        if arg.startswith("--large-mb="):
            large_mb = int(arg.split("=", 1)[1])

    targets = {
        "sample.pdf": build_pdf(),
        "sample.csv": build_csv(),
        "sample-chinese.zip": build_zip(),
        "large-sample.txt": build_large_text(large_mb),
    }
    for name, data in targets.items():
        path = os.path.join(HERE, name)
        with open(path, "wb") as f:
            f.write(data)
        print(f"wrote {path} ({len(data)} bytes)")


if __name__ == "__main__":
    main()