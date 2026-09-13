package io.github.jferments.ostudy.platform

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbAccessory
import android.hardware.usb.UsbManager
import android.os.Build
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.util.Log
import android.webkit.WebView
import androidx.core.content.ContextCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val OSTUDY_ACCESSORY_MANUFACTURER = "oStudy"
private const val OSTUDY_ACCESSORY_MODEL = "oStudy USB Sync"
private const val USB_PERMISSION_ACTION_SUFFIX = ".ostudy.USB_ACCESSORY_PERMISSION"
private const val USB_DIAGNOSTIC_TAG = "oStudyUsb"
private const val EMPTY_CLAIM_LOG_INTERVAL_MILLIS = 15_000L

/** Android Open Accessory adapter. Discovery is passive until the user accepts in oStudy. */
@TauriPlugin
class OstudyUsbPlugin(private val activity: Activity) : Plugin(activity) {
    private val lock = Any()
    private val manager by lazy { activity.getSystemService(Context.USB_SERVICE) as UsbManager }
    private val permissionAction = "${activity.packageName}$USB_PERMISSION_ACTION_SUFFIX"
    private var pending: PendingAccessory? = null
    private var suppressedKey: String? = null
    private var permissionInvoke: Invoke? = null
    private var permissionInFlight = false
    private var receiverRegistered = false
    private var lifecycleObserver: DefaultLifecycleObserver? = null
    private var lastEmptyClaimLogElapsed = -EMPTY_CLAIM_LOG_INTERVAL_MILLIS

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action != permissionAction) return
            val callback = synchronized(lock) {
                permissionInFlight = false
                val value = permissionInvoke
                permissionInvoke = null
                value
            }
            val accessory = accessoryFromIntent(intent)
            val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
            if (!granted || accessory == null || !isOstudyAccessory(accessory) || !manager.hasPermission(accessory)) {
                callback?.resolve(permissionResult(accessory != null, false))
                return
            }
            try {
                callback?.resolve(permissionResult(true, prepareAccessory(accessory, announce = true)))
            } catch (error: Exception) {
                callback?.reject("Android granted USB access but oStudy could not open the accessory: ${safeMessage(error)}")
            }
        }
    }

    override fun load(webView: WebView) {
        registerPermissionReceiver()
        installLifecycleCleanup()
        refreshSuppression()
        log("plugin_load", "sdk=${Build.VERSION.SDK_INT}")
    }

    override fun onNewIntent(intent: Intent) {
        // No USB_ACCESSORY_ATTACHED manifest handler is registered. An OEM may
        // still forward an attachment to an already-open activity; observing it
        // must never request permission before the app-level consent dialog.
        val accessory = accessoryFromIntent(intent)
        if (intent.action == UsbManager.ACTION_USB_ACCESSORY_ATTACHED && accessory != null && isOstudyAccessory(accessory)) {
            synchronized(lock) {
                if (suppressedKey != accessoryKey(accessory)) suppressedKey = null
            }
            log("accessory_intent_observed")
        }
    }

    @Command
    fun peekUsbAccessory(invoke: Invoke) {
        try {
            val opened = synchronized(lock) { pending }
            if (opened != null) {
                invoke.resolve(status(true, opened.manufacturer, opened.model, opened.serialNumber))
                return
            }
            val accessory = findDiscoverableAccessory()
            invoke.resolve(status(accessory != null, accessory?.manufacturer, accessory?.model, accessory?.let(::safeSerial)))
        } catch (error: Exception) {
            invoke.reject("The pending Android USB sync request could not be inspected: ${safeMessage(error)}")
        }
    }

    @Command
    fun requestUsbAccessoryPermission(invoke: Invoke) {
        try {
            if (synchronized(lock) { pending != null }) {
                invoke.resolve(permissionResult(true, true))
                return
            }
            val accessory = findDiscoverableAccessory()
            if (accessory == null) {
                invoke.resolve(permissionResult(false, false))
                return
            }
            if (manager.hasPermission(accessory)) {
                invoke.resolve(permissionResult(true, prepareAccessory(accessory, announce = true)))
                return
            }
            val accepted = synchronized(lock) {
                if (permissionInFlight) false else {
                    permissionInFlight = true
                    permissionInvoke = invoke
                    true
                }
            }
            if (!accepted) {
                invoke.reject("Another Android USB permission request is already active.")
                return
            }
            try {
                val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
                val permissionIntent = PendingIntent.getBroadcast(
                    activity,
                    0,
                    Intent(permissionAction).setPackage(activity.packageName),
                    flags,
                )
                manager.requestPermission(accessory, permissionIntent)
                log("permission_request_started")
            } catch (error: Exception) {
                synchronized(lock) {
                    permissionInFlight = false
                    permissionInvoke = null
                }
                throw error
            }
        } catch (error: Exception) {
            invoke.reject("Android could not request USB access for oStudy: ${safeMessage(error)}")
        }
    }

    @Command
    fun dismissUsbAccessory(invoke: Invoke) {
        try {
            val opened = synchronized(lock) { pending }
            val accessory = if (opened == null) findDiscoverableAccessory() else null
            var canRespond = opened != null
            if (!canRespond && accessory != null && manager.hasPermission(accessory)) {
                canRespond = prepareAccessory(accessory, announce = false)
            }
            val key = opened?.key ?: accessory?.let(::accessoryKey)
            synchronized(lock) { if (key != null) suppressedKey = key }
            invoke.resolve(JSObject().apply {
                put("dismissed", key != null)
                put("canRespond", canRespond)
            })
        } catch (error: Exception) {
            invoke.reject("The Android USB sync request could not be dismissed: ${safeMessage(error)}")
        }
    }

    @Command
    fun takeUsbAccessory(invoke: Invoke) {
        try {
            val current = synchronized(lock) {
                val value = pending
                pending = null
                if (value != null) suppressedKey = value.key
                value
            }
            if (current != null) {
                try {
                    OstudyUsbSyncService.begin(activity, current.fd)
                } catch (error: Exception) {
                    current.close()
                    invoke.reject("Android could not keep the USB synchronization active: ${safeMessage(error)}")
                    return
                }
            } else if (shouldLogEmptyClaim()) {
                log("take_command_no_pending_attachment")
            }
            invoke.resolve(JSObject().apply {
                put("available", current != null)
                put("fd", current?.fd)
                put("manufacturer", current?.manufacturer)
                put("model", current?.model)
                put("serialNumber", current?.serialNumber)
            })
        } catch (error: Exception) {
            invoke.reject("The Android USB accessory could not be opened: ${safeMessage(error)}")
        }
    }

    private fun findDiscoverableAccessory(): UsbAccessory? {
        val matching = manager.accessoryList.orEmpty().filter(::isOstudyAccessory)
        if (matching.isEmpty()) {
            synchronized(lock) { suppressedKey = null }
            return null
        }
        val suppressed = synchronized(lock) { suppressedKey }
        return matching.firstOrNull { accessoryKey(it) != suppressed }
    }

    private fun prepareAccessory(accessory: UsbAccessory, announce: Boolean): Boolean {
        if (!manager.hasPermission(accessory)) return false
        val descriptor = manager.openAccessory(accessory) ?: return false
        val opened = try {
            PendingAccessory(
                fd = descriptor.detachFd(),
                manufacturer = accessory.manufacturer,
                model = accessory.model,
                serialNumber = safeSerial(accessory),
                key = accessoryKey(accessory),
            )
        } finally {
            descriptor.close()
        }
        synchronized(lock) {
            pending?.close()
            pending = opened
            suppressedKey = null
        }
        if (announce) {
            trigger("usb-sync-ready", JSObject().apply {
                put("manufacturer", opened.manufacturer)
                put("model", opened.model)
                put("serialNumber", opened.serialNumber)
            })
        }
        return true
    }

    private fun refreshSuppression() {
        if (manager.accessoryList.orEmpty().none(::isOstudyAccessory)) {
            synchronized(lock) { suppressedKey = null }
        }
    }

    private fun registerPermissionReceiver() {
        if (receiverRegistered) return
        ContextCompat.registerReceiver(
            activity,
            permissionReceiver,
            IntentFilter(permissionAction),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        receiverRegistered = true
    }

    private fun installLifecycleCleanup() {
        val owner = activity as? LifecycleOwner ?: return
        val observer = object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                cleanup()
                owner.lifecycle.removeObserver(this)
            }
        }
        lifecycleObserver = observer
        owner.lifecycle.addObserver(observer)
    }

    private fun cleanup() {
        if (receiverRegistered) {
            try { activity.unregisterReceiver(permissionReceiver) } catch (_: IllegalArgumentException) {}
            receiverRegistered = false
        }
        val callback = synchronized(lock) {
            permissionInFlight = false
            val value = permissionInvoke
            permissionInvoke = null
            pending?.close()
            pending = null
            suppressedKey = null
            value
        }
        callback?.reject("The Android USB permission request ended because oStudy closed.")
        lifecycleObserver = null
    }

    private fun shouldLogEmptyClaim(): Boolean {
        val now = SystemClock.elapsedRealtime()
        return synchronized(lock) {
            if (now - lastEmptyClaimLogElapsed < EMPTY_CLAIM_LOG_INTERVAL_MILLIS) false else {
                lastEmptyClaimLogElapsed = now
                true
            }
        }
    }

    private fun status(available: Boolean, manufacturer: String?, model: String?, serial: String?) =
        JSObject().apply {
            put("available", available)
            put("manufacturer", manufacturer)
            put("model", model)
            put("serialNumber", serial)
        }

    private fun permissionResult(available: Boolean, granted: Boolean) = JSObject().apply {
        put("available", available)
        put("granted", granted)
    }

    private fun accessoryKey(accessory: UsbAccessory): String =
        listOf(accessory.manufacturer, accessory.model, accessory.version, accessory.uri)
            .joinToString("|") { it.orEmpty() }

    private fun safeSerial(accessory: UsbAccessory): String? =
        try { accessory.serial } catch (_: SecurityException) { null }

    private fun isOstudyAccessory(accessory: UsbAccessory): Boolean =
        accessory.manufacturer == OSTUDY_ACCESSORY_MANUFACTURER && accessory.model == OSTUDY_ACCESSORY_MODEL

    @Suppress("DEPRECATION")
    private fun accessoryFromIntent(intent: Intent): UsbAccessory? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(UsbManager.EXTRA_ACCESSORY, UsbAccessory::class.java)
        } else {
            intent.getParcelableExtra(UsbManager.EXTRA_ACCESSORY)
        }

    private fun log(event: String, details: String = "") {
        Log.i(USB_DIAGNOSTIC_TAG, "event=$event ${details.take(160)}".trim())
    }

    private data class PendingAccessory(
        val fd: Int,
        val manufacturer: String,
        val model: String,
        val serialNumber: String?,
        val key: String,
    ) {
        fun close() {
            if (fd < 0) return
            try { ParcelFileDescriptor.adoptFd(fd).close() } catch (_: Exception) {}
        }
    }
}

private fun safeMessage(error: Throwable): String =
    error.message?.trim()?.takeIf { it.isNotEmpty() } ?: error::class.java.simpleName
