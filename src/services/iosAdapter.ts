/**
 * iOS 文件 URL 适配层。
 *
 * iOS 上外部文件（文件 App 导入 / 分享打开）以 `file://` URL 形式进入应用
 * （Tauri `RunEvent::Opened` → Rust `emit("external-uri-open", uri)`）。
 * 本模块负责 `file://` URI 与统一 `DocumentSource`（见 `src/types`）之间的转换，
 * 并集中说明 iOS 的文件访问策略，Vue 组件层不得直接处理平台差异。
 *
 * 访问策略（P0 降级方案，详见 docs/architecture/tauri-ios-setup.md）：
 * - 默认（LSSupportsOpeningDocumentsInPlace=false，见 src-tauri/Info.ios.plist）：
 *   iOS 在用户通过"文件"App 打开文档时，会把文件**拷贝到本应用沙盒的
 *   Documents/Inbox/ 目录**，并把该拷贝的 `file://` URL 交给应用。
 *   拷贝已在沙盒内，Rust 侧 `read_file(path)` 可直接读取，无需额外权限。
 * - 原地打开（LSSupportsOpeningDocumentsInPlace=true，P1 增强）：
 *   URL 指向沙盒外的安全作用域资源（Security-Scoped Resource），必须先调用
 *   `startAccessingSecurityScopedResource` 才能读取，需要原生桥接
 *   （ios_access.rs 草稿见 Agent G 报告与 tauri-ios-setup.md 附录）。
 *
 * 注意：Security Scoped Resource 的获取/释放是原生侧（iOS）职责，
 * 前端只能拿到 `file://` URI 字符串；本模块不做任何沙盒外读取。
 */
import type { DocumentSource } from '../types'
import { getExtension, getFileName } from '../utils'
import { createSourceFromUri } from './platformService'

const FILE_SCHEME_RE = /^file:\/\//i

/**
 * 判断 URI 是否为 iOS/macOS 风格的 `file://` URL。
 *
 * @param uri 任意 URI 字符串（可能来自 external-uri-open 事件或 dialog 返回值）
 * @returns 是 file:// 协议时返回 true
 */
export function isFileUrl(uri: string): boolean {
  return FILE_SCHEME_RE.test(uri)
}

/**
 * 将 `file://` URL 解码为本地文件系统路径。
 *
 * 处理：`file://localhost/...` 形式、百分号编码（%20 空格、%E4%B8%AD 中文等）、
 * `file:///var/...` 三斜杠形式。非 file:// 输入原样返回。
 *
 * iOS 示例：
 * - `file:///var/mobile/Containers/Data/Application/<UUID>/Documents/Inbox/readme.md`
 *   → `/var/mobile/Containers/Data/Application/<UUID>/Documents/Inbox/readme.md`
 * - `file://localhost/private/var/...` → `/private/var/...`
 *
 * @param uri file:// URL
 * @returns 解码后的 POSIX 路径；解码失败时返回去除 scheme 后的原始串
 */
export function fileUrlToPath(uri: string): string {
  if (!isFileUrl(uri)) return uri
  // 去掉协议头；兼容 file://localhost/ 与 file:/// 两种写法
  let rest = uri.slice('file://'.length)
  if (rest.startsWith('localhost/')) {
    rest = rest.slice('localhost'.length)
  }
  if (!rest.startsWith('/')) {
    // file://host/path 形式（host 非 localhost）：按 file:///path 处理，丢弃 host
    const slash = rest.indexOf('/')
    rest = slash >= 0 ? rest.slice(slash) : ''
  }
  try {
    return decodeURIComponent(rest)
  } catch {
    // 非法百分号编码：退化为原始串，保证不抛异常
    return rest
  }
}

/**
 * 从 `file://` URL 构造统一的 DocumentSource（移动端约定）。
 *
 * - `source.uri` 保留**原始** file:// URI（合同第 10 节：iOS file:// URL → DocumentSource.uri），
 *   作为文档唯一标识（getDocumentIdentity 原样归一）与"近期文件"标识。
 * - `source.name` / `source.extension` 从解码后的路径提取，避免 %20 等编码泄漏到 UI。
 * - 若调用方已知解码后的沙盒路径（如 Inbox 内），可写入 `source.path` 供 Rust
 *   `read_file` 直接使用（见 fileUrlToPath）。
 *
 * @param uri file:// URL（来自 external-uri-open 事件或启动参数）
 * @param mimeType 可选 MIME 类型（分享打开时系统可能提供）
 */
export function fileUrlToSource(uri: string, mimeType?: string): DocumentSource {
  const source = createSourceFromUri(uri, mimeType)
  const path = fileUrlToPath(uri)
  if (path) {
    source.name = getFileName(path)
    source.extension = getExtension(path)
  }
  return source
}