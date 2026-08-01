package com.featherview.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream
import org.json.JSONObject

/**
 * FeatherView Android 原生桥接：SAF content:// URI 读取 + 外部打开/分享分发。
 *
 * 注册：Rust 侧插件 setup 中 `api.register_android_plugin("com.featherview.app", "FeatherViewPlugin")`
 * （注册代码见 Agent F 报告 §4）。类全名 = <identifier 包路径>.FeatherViewPlugin，必须与
 * tauri.conf.json 的 identifier 一致；tauri 以 `(Landroid/app/Activity;)V` 构造器实例化。
 *
 * 命令（Rust → Kotlin，经 PluginHandle.run_mobile_plugin / run_mobile_plugin_async）：
 *   - cacheDir()            -> { cacheDir: String }   应用缓存目录（featherview-uris 绝对路径）
 *   - copyToTemp(uri)       -> { path, size, name, mimeType, cacheDir, error? }
 *       把 content:// 流式拷贝到 cacheDir/featherview-uris/<fnv1a64(uri)><ext>，
 *       供 Rust 侧 read_uri 按普通文件分块读取（临时文件拷贝策略，合同 §9）。
 *
 * 事件（Kotlin → 前端，合同 §11 external-uri-open）：
 *   - onNewIntent：ACTION_VIEW（"打开方式"）与 ACTION_SEND（分享）的 content:// uri
 *     经双通道送达：① trigger("newIntent", {uri})（Tauri 插件事件，需 capability 允许监听）
 *     ② evaluateJavascript("window.__featherviewAndroidUri(uri)")（无权限依赖，主通道，
 *     前端 androidAdapter.listenUriOpen 注册）。
 */
@InvokeArg
internal class CopyToTempArgs {
    lateinit var uri: String
}

@TauriPlugin
class FeatherViewPlugin(private val activity: Activity) : Plugin(activity) {

    private var webView: WebView? = null

    override fun load(webView: WebView) {
        this.webView = webView
    }

    // ---- 生命周期：外部打开 / 分享（singleTask 复用时走这里；冷启动 ACTION_VIEW 由 onCreate 后 load 分发） ----
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        dispatchIntent(intent)
    }

    private fun dispatchIntent(intent: Intent) {
        val uri = intent.data?.toString()
            ?: intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.toString()
            ?: return
        // 通道 1：Tauri 插件事件（未配置 capability 时前端收不到，见通道 2）
        val event = JSObject()
        event.put("uri", uri)
        trigger("newIntent", event)
        // 通道 2：直接调用前端全局函数（无权限依赖）
        webView?.post {
            val escaped = JSONObject.quote(uri)
            webView?.evaluateJavascript(
                "window.__featherviewAndroidUri && window.__featherviewAndroidUri($escaped)",
                null
            )
        }
    }

    // ---- 命令：返回缓存目录（供 Rust 侧磁盘重建扫描） ----
    @Command
    fun cacheDir(invoke: Invoke) {
        val dir = uriCacheDir()
        dir.mkdirs()
        val ret = JSObject()
        ret.put("cacheDir", dir.absolutePath)
        invoke.resolve(ret)
    }

    // ---- 命令：拷贝 content:// 到应用缓存（临时文件策略） ----
    @Command
    fun copyToTemp(invoke: Invoke) {
        val args = invoke.parseArgs(CopyToTempArgs::class.java)
        val ret = copyUriToCache(Uri.parse(args.uri))
        invoke.resolve(ret)
    }

    private fun uriCacheDir(): File = File(activity.cacheDir, "featherview-uris")

    private fun copyUriToCache(uri: Uri): JSObject {
        val ret = JSObject()
        try {
            val resolver = activity.contentResolver
            val mimeType = resolver.getType(uri)
            val input = resolver.openInputStream(uri)
                ?: throw IllegalStateException("无法打开输入流：$uri")
            val dir = uriCacheDir().apply { mkdirs() }
            val target = File(dir, cacheFileName(uri.toString()) + extensionFor(uri, mimeType))
            input.use { ins ->
                FileOutputStream(target).use { out -> ins.copyTo(out) }
            }
            // 尝试持久化 SAF 授权（DocumentProvider 支持时）；失败则仅本次进程授权期内可读
            try {
                resolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            } catch (_: Exception) {
                // 忽略：临时授权也可用
            }
            ret.put("path", target.absolutePath)
            ret.put("size", target.length())
            ret.put("name", queryDisplayName(uri) ?: target.name)
            ret.put("mimeType", mimeType ?: "")
            ret.put("cacheDir", dir.absolutePath)
        } catch (e: Exception) {
            ret.put("error", e.message ?: e.toString())
        }
        return ret
    }

    private fun queryDisplayName(uri: Uri): String? = try {
        activity.contentResolver.query(
            uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null
        )?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }
    } catch (_: Exception) {
        null
    }

    private fun extensionFor(uri: Uri, mimeType: String?): String {
        val fromMime = when (mimeType?.substringBefore(';')?.lowercase()) {
            "text/markdown" -> "md"
            "text/plain" -> "txt"
            "application/json" -> "json"
            "application/x-yaml", "text/yaml" -> "yaml"
            "application/toml" -> "toml"
            "application/xml", "text/xml" -> "xml"
            "text/html" -> "html"
            "text/css" -> "css"
            "application/pdf" -> "pdf"
            "image/png" -> "png"
            "image/jpeg" -> "jpg"
            "image/webp" -> "webp"
            "image/gif" -> "gif"
            "image/svg+xml" -> "svg"
            else -> null
        }
        if (fromMime != null) return ".$fromMime"
        // 兜底：从 URI 末段取扩展名（限长防异常值）
        val last = uri.lastPathSegment?.substringAfterLast('.', "")
        return if (!last.isNullOrEmpty() && last.length <= 8) ".$last" else ""
    }

    /** FNV-1a 64（与 Rust 侧 uri.rs::cache_file_name 位模式一致；Long 溢出自动回绕） */
    private fun cacheFileName(uri: String): String {
        var hash = -3750763034362895579L // 0xcbf29ce484222325
        for (b in uri.toByteArray(Charsets.UTF_8)) {
            hash = hash xor b.toLong()
            hash *= 0x100000001b3L       // FNV prime = 1099511628211
        }
        return java.lang.Long.toHexString(hash).padStart(16, '0')
    }
}