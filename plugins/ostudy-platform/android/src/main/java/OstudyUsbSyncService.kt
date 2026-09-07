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
import android.os.Handler
import android.os.IBinder
import android.os.ParcelFileDescriptor
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat

private const val USB_SYNC_CHANNEL_ID = "ostudy_usb_sync"
private const val USB_SYNC_NOTIFICATION_ID = 0x4f54
private const val ACTION_BEGIN_USB_SYNC = "io.github.jferments.ostudy.platform.BEGIN_USB_SYNC"
private const val EXTRA_ACCESSORY_FD = "accessory_fd"
private const val USB_SYNC_FD_CHECK_INTERVAL_MILLIS = 1_500L
private const val MAX_USB_SYNC_RUNTIME_MILLIS = 2L * 60L * 60L * 1_000L
private const val USB_SYNC_SERVICE_TAG = "oStudyUsbService"

/**
 * Android process-lifetime envelope for an active USB accessory transfer.
 *
 * Rust owns authentication, framing, cancellation, package validation, merge,
 * and the descriptor itself. This service exists only so Android keeps a
 * user-initiated transfer alive when the activity is backgrounded. A partial
 * wake lock covers the same bounded transfer lifetime. The service watches a
 * duplicate of the process-local raw descriptor and stops itself as soon as
 * Rust closes that descriptor at the end of the synchronization session.
 */
class OstudyUsbSyncService : Service() {
    private var accessoryFd: Int? = null
    private var startedAtElapsedMillis = 0L
    private var wakeLock: PowerManager.WakeLock? = null
    private val handler by lazy { Handler(mainLooper) }

    private val descriptorMonitor = object : Runnable {
        override fun run() {
            val fd = accessoryFd
            if (fd == null) {
                stopTransfer("missing-descriptor")
                return
            }
            if (SystemClock.elapsedRealtime() - startedAtElapsedMillis >= MAX_USB_SYNC_RUNTIME_MILLIS) {
                Log.w(USB_SYNC_SERVICE_TAG, "USB synchronization exceeded the foreground-service safety limit")
                stopTransfer("runtime-limit")
                return
            }
            if (!descriptorIsOpen(fd)) {
                stopTransfer("descriptor-closed")
                return
            }
            handler.postDelayed(this, USB_SYNC_FD_CHECK_INTERVAL_MILLIS)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action != ACTION_BEGIN_USB_SYNC) {
            stopSelf(startId)
            return START_NOT_STICKY
        }
        val fd = intent.getIntExtra(EXTRA_ACCESSORY_FD, -1)
        if (fd < 0 || !descriptorIsOpen(fd)) {
            Log.w(USB_SYNC_SERVICE_TAG, "Refusing to guard an invalid USB accessory descriptor")
            stopSelf(startId)
            return START_NOT_STICKY
        }
        beginTransfer(fd)
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        handler.removeCallbacks(descriptorMonitor)
        accessoryFd = null
        releaseWakeLock()
        super.onDestroy()
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        Log.w(
            USB_SYNC_SERVICE_TAG,
            "Android ended the USB dataSync foreground-service allowance: startId=$startId type=$fgsType",
        )
        stopTransfer("android-timeout")
    }

    private fun beginTransfer(fd: Int) {
        accessoryFd = fd
        startedAtElapsedMillis = SystemClock.elapsedRealtime()
        acquireWakeLock()
        ServiceCompat.startForeground(
            this,
            USB_SYNC_NOTIFICATION_ID,
            buildNotification(),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
        handler.removeCallbacks(descriptorMonitor)
        handler.postDelayed(descriptorMonitor, USB_SYNC_FD_CHECK_INTERVAL_MILLIS)
        Log.i(USB_SYNC_SERVICE_TAG, "USB synchronization foreground guard started")
    }

    private fun stopTransfer(reason: String) {
        handler.removeCallbacks(descriptorMonitor)
        accessoryFd = null
        releaseWakeLock()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
        Log.i(USB_SYNC_SERVICE_TAG, "USB synchronization foreground guard stopped: $reason")
    }

    private fun buildNotification(): Notification {
        val contentIntent = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(
                this,
                0,
                it.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val builder = NotificationCompat.Builder(this, USB_SYNC_CHANNEL_ID)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Synchronizing study data over USB")
            .setContentText("Keep the Android phone connected while oStudy finishes the transfer.")
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setProgress(0, 0, true)
        contentIntent?.let(builder::setContentIntent)
        return builder.build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        notificationManager().createNotificationChannel(
            NotificationChannel(
                USB_SYNC_CHANNEL_ID,
                "USB synchronization",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Progress while oStudy synchronizes study data over an attached USB cable"
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
            "$packageName:usb-sync",
        ).apply {
            setReferenceCounted(false)
            acquire(MAX_USB_SYNC_RUNTIME_MILLIS)
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let { if (it.isHeld) it.release() }
        wakeLock = null
    }

    private fun descriptorIsOpen(fd: Int): Boolean = try {
        ParcelFileDescriptor.fromFd(fd).use { }
        true
    } catch (_: Exception) {
        false
    }

    companion object {
        fun begin(context: Context, accessoryFd: Int) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, OstudyUsbSyncService::class.java)
                    .setAction(ACTION_BEGIN_USB_SYNC)
                    .putExtra(EXTRA_ACCESSORY_FD, accessoryFd),
            )
        }
    }
}
