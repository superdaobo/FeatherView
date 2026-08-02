/**
 * 启动参数解析（纯函数，可测试）。
 *
 * 规则：
 * - 丢弃可执行文件自身路径（首个参数）与 `-`/`/` 开头的 Flag
 * - 去除首尾引号（防御：std::env::args 通常已去引号）
 * - 逐个校验真实文件（排除不存在路径、文件夹、URL）
 * - 路径大小写不敏感去重
 * - 多个有效文件全部返回（由调用方决定只取第一个）
 */

export interface LaunchParseResult {
  /** 规范化后的有效文件路径（保持传入顺序，去重） */
  files: string[]
  /** 被忽略的 Flag 数量 */
  flags: number
  /** 被忽略的非文件参数数量（不存在/文件夹/URL 等） */
  ignored: number
}

function isFlag(arg: string): boolean {
  return arg.startsWith('-') || arg.startsWith('/')
}

/** 去除首尾成对引号（仅当确实成对时） */
export function stripQuotes(arg: string): string {
  let value = arg.trim()
  while (value.length >= 2) {
    const first = value[0]
    const last = value[value.length - 1]
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) {
      value = value.slice(1, -1).trim()
    } else {
      break
    }
  }
  return value
}

/** Windows 路径大小写不敏感去重 */
function dedupePaths(paths: string[]): string[] {
  const seen = new Set<string>()
  const result: string[] = []
  for (const p of paths) {
    const key = p.toLowerCase()
    if (!seen.has(key)) {
      seen.add(key)
      result.push(p)
    }
  }
  return result
}

/**
 * 解析启动参数。
 * @param args 原始参数数组（含 exe 自身路径）
 * @param exists 判断路径是否为真实文件（注入以便测试）
 */
export async function parseLaunchArgs(
  args: string[],
  exists: (path: string) => Promise<boolean>,
): Promise<LaunchParseResult> {
  const files: string[] = []
  let flags = 0
  let ignored = 0

  // 首个参数通常是可执行文件自身路径，丢弃
  for (const raw of args.slice(1)) {
    if (!raw) continue
    if (isFlag(raw)) {
      flags++
      continue
    }
    const candidate = stripQuotes(raw)
    if (!candidate) {
      ignored++
      continue
    }
    // URL 或明显非文件路径直接忽略
    if (/^[a-z]+:\/\//i.test(candidate)) {
      ignored++
      continue
    }
    let isFile = false
    try {
      isFile = await exists(candidate)
    } catch {
      isFile = false
    }
    if (isFile) {
      files.push(candidate)
    } else {
      ignored++
    }
  }

  return { files: dedupePaths(files), flags, ignored }
}
