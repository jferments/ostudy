package io.github.jferments.ostudy.platform

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import java.util.LinkedHashMap

private const val DOCUMENT_CHANNEL_ID = "ostudy_document_processing"
private const val DOCUMENT_NOTIFICATION_ID = 0x4f53
private const val ACTION_BEGIN = "io.github.jferments.ostudy.platform.BEGIN_DOCUMENT_EXECUTION"
private const val ACTION_UPDATE = "io.github.jferments.ostudy.platform.UPDATE_DOCUMENT_EXECUTION"
private const val ACTION_END = "io.github.jferments.ostudy.platform.END_DOCUMENT_EXECUTION"
private const val EXTRA_JOB_ID = "job_id"
private const val EXTRA_TITLE = "title"
private const val EXTRA_DETAIL = "detail"
private const val EXTRA_PROGRESS = "progress"

class OstudyDocumentExecutionService : Service() {
    private val jobs = LinkedHashMap<String, JobState>()
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_BEGIN -> begin(intent)
            ACTION_UPDATE -> update(intent)
            ACTION_END -> end(intent)
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        synchronized(jobs) { jobs.clear() }
        releaseWakeLock()
        super.onDestroy()
    }

    private fun begin(intent: Intent) {
        val jobId = requiredJobId(intent) ?: return
        val title = intent.getStringExtra(EXTRA_TITLE)?.take(120).orEmpty()
            .ifBlank { "Processing study document" }
        val detail = intent.getStringExtra(EXTRA_DETAIL)?.take(240).orEmpty()
            .ifBlank { "Preparing PDF pages" }
        synchronized(jobs) { jobs[jobId] = JobState(title, detail, null) }
        acquireWakeLock()
        ServiceCompat.startForeground(
            this,
            DOCUMENT_NOTIFICATION_ID,
            buildNotification(currentJob()),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
    }

    private fun update(intent: Intent) {
        val jobId = requiredJobId(intent) ?: return
        synchronized(jobs) {
            val current = jobs[jobId] ?: return
            val detail = intent.getStringExtra(EXTRA_DETAIL)?.take(240)
                ?.takeIf(String::isNotBlank) ?: current.detail
            val rawProgress = intent.getIntExtra(EXTRA_PROGRESS, -1)
            jobs[jobId] = current.copy(
                detail = detail,
                progress = rawProgress.takeIf { it in 0..100 },
            )
        }
        notificationManager().notify(DOCUMENT_NOTIFICATION_ID, buildNotification(currentJob()))
    }

    private fun end(intent: Intent) {
        val jobId = requiredJobId(intent) ?: return
        val empty = synchronized(jobs) {
            jobs.remove(jobId)
            jobs.isEmpty()
        }
        if (empty) {
            releaseWakeLock()
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            stopSelf()
        } else {
            notificationManager().notify(DOCUMENT_NOTIFICATION_ID, buildNotification(currentJob()))
        }
    }

    private fun currentJob(): JobState = synchronized(jobs) {
        jobs.values.lastOrNull()
            ?: JobState("Processing study document", "Working in the background", null)
    }

    private fun buildNotification(job: JobState): Notification {
        val contentIntent = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(
                this,
                0,
                it.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val builder = NotificationCompat.Builder(this, DOCUMENT_CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle(job.title)
            .setContentText(job.detail)
            .setStyle(NotificationCompat.BigTextStyle().bigText(job.detail))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
        contentIntent?.let(builder::setContentIntent)
        if (job.progress == null) builder.setProgress(0, 0, true)
        else builder.setProgress(100, job.progress, false)
        return builder.build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        notificationManager().createNotificationChannel(
            NotificationChannel(
                DOCUMENT_CHANNEL_ID,
                "Document processing",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Progress for PDF rendering, OCR, and study-material generation"
                setShowBadge(false)
            },
        )
    }

    private fun notificationManager(): NotificationManager =
        getSystemService(NotificationManager::class.java)

    private fun acquireWakeLock() {
        if (wakeLock?.isHeld == true) return
        val manager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = manager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "$packageName:document-processing",
        ).apply {
            setReferenceCounted(false)
            acquire()
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
    }

    private fun requiredJobId(intent: Intent): String? =
        intent.getStringExtra(EXTRA_JOB_ID)
            ?.takeIf { it.matches(Regex("[A-Za-z0-9_-]{1,128}")) }

    private data class JobState(
        val title: String,
        val detail: String,
        val progress: Int?,
    )

    companion object {
        fun begin(context: Context, jobId: String, title: String, detail: String) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, OstudyDocumentExecutionService::class.java)
                    .setAction(ACTION_BEGIN)
                    .putExtra(EXTRA_JOB_ID, jobId)
                    .putExtra(EXTRA_TITLE, title)
                    .putExtra(EXTRA_DETAIL, detail),
            )
        }

        fun update(context: Context, jobId: String, detail: String, progress: Int?) {
            val intent = Intent(context, OstudyDocumentExecutionService::class.java)
                .setAction(ACTION_UPDATE)
                .putExtra(EXTRA_JOB_ID, jobId)
                .putExtra(EXTRA_DETAIL, detail)
            progress?.let { intent.putExtra(EXTRA_PROGRESS, it.coerceIn(0, 100)) }
            context.startService(intent)
        }

        fun end(context: Context, jobId: String) {
            context.startService(
                Intent(context, OstudyDocumentExecutionService::class.java)
                    .setAction(ACTION_END)
                    .putExtra(EXTRA_JOB_ID, jobId),
            )
        }
    }
}
