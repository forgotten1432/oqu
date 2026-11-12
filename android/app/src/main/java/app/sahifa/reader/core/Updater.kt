package app.sahifa.reader.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.FileProvider
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import app.sahifa.reader.BuildConfig
import app.sahifa.reader.MainActivity
import app.sahifa.reader.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

/**
 * Self-update from GitHub Releases. The only network access the app makes:
 * it reads the latest release, downloads the APK built for this device's CPU,
 * checks it against SHA256SUMS.txt and hands it to Android's installer.
 */
object Updater {
    private const val API = "https://api.github.com/repos/forgotten1432/oqu/releases/latest"
    const val CHANNEL = "updates"
    const val EXTRA_SHOW_UPDATE = "show_update"

    data class Release(
        val version: String,
        val notes: String,
        val apkName: String,
        val apkUrl: String,
        val apkSize: Long,
        val sumsUrl: String?,
        val pageUrl: String,
    )

    var autoCheck: Boolean
        get() = Core.setting("update_auto", "1") == "1"
        set(v) = Core.setSetting("update_auto", if (v) "1" else "0")

    val currentVersion: String get() = BuildConfig.VERSION_NAME

    /** "0.10.2" > "0.9.9"; non-numeric suffixes are ignored. */
    fun isNewer(remote: String, local: String): Boolean {
        fun parts(v: String) = v.trim().removePrefix("v").split('.', '-').map { it.takeWhile(Char::isDigit).toIntOrNull() ?: 0 }
        val a = parts(remote)
        val b = parts(local)
        for (i in 0 until maxOf(a.size, b.size)) {
            val x = a.getOrElse(i) { 0 }
            val y = b.getOrElse(i) { 0 }
            if (x != y) return x > y
        }
        return false
    }

    private fun open(url: String): HttpURLConnection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("Accept", "application/vnd.github+json")
            setRequestProperty("User-Agent", "Oqu-Android/${BuildConfig.VERSION_NAME}")
        }

    /** Latest release if it is newer than the installed app, otherwise null. */
    suspend fun check(): Release? = withContext(Dispatchers.IO) {
        val conn = open(API)
        try {
            if (conn.responseCode != 200) return@withContext null
            val json = JSONObject(conn.inputStream.bufferedReader().readText())
            val version = json.optString("tag_name").removePrefix("v")
            Core.setSetting("update_last_check", System.currentTimeMillis().toString())
            if (version.isBlank() || !isNewer(version, currentVersion)) return@withContext null
            val assets = json.optJSONArray("assets") ?: return@withContext null
            val list = (0 until assets.length()).map { assets.getJSONObject(it) }
            fun find(pred: (String) -> Boolean) = list.firstOrNull { pred(it.optString("name")) }
            // the APK built for this CPU (64-bit ARM on almost every current device)
            val abi = Build.SUPPORTED_ABIS.firstOrNull() ?: "arm64-v8a"
            val apk = find { it.endsWith(".apk") && it.contains(abi) && !(abi == "arm64-v8a" && it.contains("armeabi")) }
                ?: find { it.endsWith(".apk") && it.contains("universal") }
                ?: return@withContext null
            Release(
                version = version,
                notes = json.optString("body"),
                apkName = apk.optString("name"),
                apkUrl = apk.optString("browser_download_url"),
                apkSize = apk.optLong("size"),
                sumsUrl = find { it == "SHA256SUMS.txt" }?.optString("browser_download_url"),
                pageUrl = json.optString("html_url"),
            )
        } finally {
            conn.disconnect()
        }
    }

    /** Downloads the APK into app cache and verifies its SHA-256 when a checksum list exists. */
    suspend fun download(r: Release, onProgress: (Float) -> Unit): File = withContext(Dispatchers.IO) {
        val dir = File(Core.app.cacheDir, "updates").apply { mkdirs() }
        dir.listFiles()?.forEach { if (it.name != r.apkName) it.delete() }
        val out = File(dir, r.apkName)
        val md = MessageDigest.getInstance("SHA-256")
        val conn = open(r.apkUrl)
        try {
            if (conn.responseCode != 200) throw java.io.IOException("HTTP ${conn.responseCode}")
            val total = conn.contentLengthLong.takeIf { it > 0 } ?: r.apkSize
            conn.inputStream.use { input ->
                out.outputStream().use { o ->
                    val buf = ByteArray(64 * 1024)
                    var done = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        o.write(buf, 0, n)
                        md.update(buf, 0, n)
                        done += n
                        if (total > 0) onProgress((done.toFloat() / total).coerceIn(0f, 1f))
                    }
                }
            }
        } finally {
            conn.disconnect()
        }
        r.sumsUrl?.let { url ->
            val sums = open(url).let { c -> try { c.inputStream.bufferedReader().readText() } finally { c.disconnect() } }
            val expected = sums.lines().firstOrNull { it.trim().endsWith(r.apkName) }?.trim()?.substringBefore(' ')?.lowercase()
            val actual = md.digest().joinToString("") { "%02x".format(it) }
            if (expected != null && expected != actual) {
                out.delete()
                throw SecurityException("checksum mismatch")
            }
        }
        out
    }

    /**
     * Opens Android's installer. The first time, Android asks the user to allow
     * this app to install updates; returns false in that case so the UI can explain.
     */
    fun install(ctx: Context, apk: File): Boolean {
        if (Build.VERSION.SDK_INT >= 26 && !ctx.packageManager.canRequestPackageInstalls()) {
            ctx.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${ctx.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            return false
        }
        val uri = FileProvider.getUriForFile(ctx, ctx.packageName + ".files", apk)
        ctx.startActivity(
            Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
        )
        return true
    }

    /** Daily background check (only when online); posts a notification for new versions. */
    fun schedule(ctx: Context) {
        val wm = WorkManager.getInstance(ctx)
        if (!autoCheck) {
            wm.cancelUniqueWork("update-check")
            return
        }
        val req = PeriodicWorkRequestBuilder<UpdateWorker>(1, TimeUnit.DAYS)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        wm.enqueueUniquePeriodicWork("update-check", ExistingPeriodicWorkPolicy.KEEP, req)
    }

    fun notify(ctx: Context, r: Release) {
        // one notification per version
        if (Core.setting("update_notified", "") == r.version) return
        val nm = ctx.getSystemService(NotificationManager::class.java)
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(NotificationChannel(CHANNEL, ctx.getString(R.string.updates), NotificationManager.IMPORTANCE_DEFAULT))
        }
        val tap = PendingIntent.getActivity(
            ctx, 1,
            Intent(ctx, MainActivity::class.java).putExtra(EXTRA_SHOW_UPDATE, true).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val n = NotificationCompat.Builder(ctx, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .setContentTitle(ctx.getString(R.string.update_notification_title, r.version, ctx.getString(R.string.app_name)))
            .setContentText(ctx.getString(R.string.update_notification_body))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        runCatching { NotificationManagerCompat.from(ctx).notify(42, n) }
        Core.setSetting("update_notified", r.version)
    }
}

class UpdateWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        if (!Updater.autoCheck) return Result.success()
        return try {
            Updater.check()?.let { Updater.notify(applicationContext, it) }
            Result.success()
        } catch (t: Throwable) {
            Result.retry()
        }
    }
}
