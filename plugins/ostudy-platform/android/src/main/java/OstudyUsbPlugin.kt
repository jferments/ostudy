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
import android.webkit.WebView
import androidx.core.content.ContextCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val OSTUDY_ACCESSORY_MANUFACTURER = "oStudy"
private const val OSTUDY_ACCESSORY_MODEL = "oStudy USB Sync"
private const val USB_PERMISSION_ACTION_SUFFIX = ".ostudy.USB_ACCESSORY_PERMISSION"

/**
 * Owns only Android's USB-accessory lifecycle and native file-descriptor handoff.
 *
 * The desktop is the AOA USB host. Android opens the accessory and transfers
 * ownership of the resulting raw descriptor to Rust exactly once. Framing,
 * identity, encryption, package validation, merge policy, and cancellation stay
 * in the shared Rust device-sync implementation.
 */
@TauriPlugin
class OstudyUsbPlugin(private val activity: Activity) : Plugin(activity) {
    private val lock = Any()
    private val manager: UsbManager by lazy {
        activity.getSystemService(Context.USB_SERVICE) as UsbManager
    }
    private val permissionAction = "${activity.packageName}$USB_PERMISSION_ACTION_SUFFIX"
    private var pending: PendingAccessory? = null
    private var permissionRequestInFlight = false
    private var permissionReceiverRegistered = false
    private var lifecycleObserver: DefaultLifecycleObserver? = null

    private val permissionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action != permissionAction) return
            synchronized(lock) {
                permissionRequestInFlight = false
            }

            val accessory = accessoryFromIntent(intent)
                ?: manager.accessoryList?.firstOrNull(::isOstudyAccessory)
            val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
            if (!granted || accessory == null || !isOstudyAccessory(accessory)) {
                Logger.warn("Android USB accessory permission was not granted for oStudy USB Sync")
                return
            }

            prepareAccessory(manager, accessory, announce = true)
        }
    }

    override fun load(webView: WebView) {
        registerPermissionReceiver()
        installLifecycleCleanup()
        processAccessoryIntent(activity.intent, announce = true)
    }

    override fun onNewIntent(intent: Intent) {
        processAccessoryIntent(intent, announce = true)
    }

    @Command
    fun takeUsbAccessory(invoke: Invoke) {
        try {
            var current = synchronized(lock) {
                val value = pending
                pending = null
                value
            }
            if (current == null) {
                val accessory = manager.accessoryList
                    ?.firstOrNull(::isOstudyAccessory)
                if (accessory != null) {
                    if (manager.hasPermission(accessory)) {
                        current = openAccessory(manager, accessory)
                    } else {
                        requestAccessoryPermission(manager, accessory)
                    }
                }
            }

            val result = JSObject()
            if (current == null) {
                result.put("available", false)
                result.put("fd", null)
                result.put("manufacturer", null)
                result.put("model", null)
                result.put("serialNumber", null)
            } else {
                result.put("available", true)
                result.put("fd", current.fd)
                result.put("manufacturer", current.manufacturer)
                result.put("model", current.model)
                result.put("serialNumber", current.serialNumber)
            }
            invoke.resolve(result)
        } catch (error: Exception) {
            invoke.reject("The Android USB accessory could not be opened: ${safeUsbMessage(error)}")
        }
    }

    private fun processAccessoryIntent(intent: Intent?, announce: Boolean) {
        if (intent?.action != UsbManager.ACTION_USB_ACCESSORY_ATTACHED) return
        val accessory = accessoryFromIntent(intent) ?: return
        if (!isOstudyAccessory(accessory)) return

        try {
            if (!manager.hasPermission(accessory)) {
                requestAccessoryPermission(manager, accessory)
                return
            }
            prepareAccessory(manager, accessory, announce)
        } catch (error: Exception) {
            Logger.error("Unable to prepare the oStudy USB accessory: ${safeUsbMessage(error)}")
        }
    }

    private fun requestAccessoryPermission(manager: UsbManager, accessory: UsbAccessory) {
        if (manager.hasPermission(accessory)) {
            prepareAccessory(manager, accessory, announce = true)
            return
        }
        synchronized(lock) {
            if (permissionRequestInFlight) return
            permissionRequestInFlight = true
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
            Logger.info("Requesting Android permission for oStudy USB Sync accessory")
            manager.requestPermission(accessory, permissionIntent)
        } catch (error: Exception) {
            synchronized(lock) {
                permissionRequestInFlight = false
            }
            throw error
        }
    }

    private fun prepareAccessory(
        manager: UsbManager,
        accessory: UsbAccessory,
        announce: Boolean,
    ) {
        if (!manager.hasPermission(accessory)) return
        val opened = openAccessory(manager, accessory) ?: return
        synchronized(lock) {
            pending?.close()
            pending = opened
        }
        if (announce) {
            val event = JSObject()
            event.put("manufacturer", opened.manufacturer)
            event.put("model", opened.model)
            event.put("serialNumber", opened.serialNumber)
            trigger("usb-sync-ready", event)
        }
    }

    private fun openAccessory(
        manager: UsbManager,
        accessory: UsbAccessory,
    ): PendingAccessory? {
        val descriptor = manager.openAccessory(accessory) ?: return null
        return try {
            val rawFd = descriptor.detachFd()
            PendingAccessory(
                fd = rawFd,
                manufacturer = accessory.manufacturer ?: OSTUDY_ACCESSORY_MANUFACTURER,
                model = accessory.model ?: OSTUDY_ACCESSORY_MODEL,
                serialNumber = accessory.serial,
            )
        } finally {
            descriptor.close()
        }
    }

    private fun registerPermissionReceiver() {
        if (permissionReceiverRegistered) return
        ContextCompat.registerReceiver(
            activity,
            permissionReceiver,
            IntentFilter(permissionAction),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        permissionReceiverRegistered = true
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
        if (permissionReceiverRegistered) {
            try {
                activity.unregisterReceiver(permissionReceiver)
            } catch (_: IllegalArgumentException) {
                // Best-effort cleanup during Activity teardown.
            }
            permissionReceiverRegistered = false
        }
        synchronized(lock) {
            permissionRequestInFlight = false
            pending?.close()
            pending = null
        }
        lifecycleObserver = null
    }

    private fun isOstudyAccessory(accessory: UsbAccessory): Boolean =
        accessory.manufacturer == OSTUDY_ACCESSORY_MANUFACTURER &&
            accessory.model == OSTUDY_ACCESSORY_MODEL

    @Suppress("DEPRECATION")
    private fun accessoryFromIntent(intent: Intent): UsbAccessory? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(UsbManager.EXTRA_ACCESSORY, UsbAccessory::class.java)
        } else {
            intent.getParcelableExtra(UsbManager.EXTRA_ACCESSORY)
        }

    private data class PendingAccessory(
        val fd: Int,
        val manufacturer: String,
        val model: String,
        val serialNumber: String?,
    ) {
        fun close() {
            if (fd >= 0) {
                try {
                    ParcelFileDescriptor.adoptFd(fd).close()
                } catch (_: Exception) {
                    // The descriptor is best-effort cleanup for an unclaimed
                    // attachment; ownership never returns here after take.
                }
            }
        }
    }
}

private fun safeUsbMessage(error: Throwable): String =
    error.message?.trim()?.takeIf { it.isNotEmpty() } ?: error::class.java.simpleName
