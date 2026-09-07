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
import android.util.Log
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
private const val USB_DIAGNOSTIC_TAG = "oStudyUsb"

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

            // requestPermission() is specified to return the exact accessory in
            // EXTRA_ACCESSORY. Never substitute UsbManager.accessoryList here:
            // Android can retain a stale logical accessory after a failed AOA
            // transition even though the host has already returned to MTP/ADB.
            val accessory = accessoryFromIntent(intent)
            val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
            val hasPermission = accessory != null && manager.hasPermission(accessory)
            logUsb(
                Log.INFO,
                "permission_result",
                "granted=$granted accessoryPresent=${accessory != null} " +
                    "matches=${accessory?.let(::isOstudyAccessory) == true} " +
                    "hasPermission=$hasPermission",
            )
            logAccessorySnapshot("permission-result")
            if (!granted || accessory == null || !isOstudyAccessory(accessory)) {
                Logger.warn("Android USB accessory permission was not granted for oStudy USB Sync")
                return
            }

            prepareAccessory(manager, accessory, announce = true)
        }
    }

    override fun load(webView: WebView) {
        logUsb(
            Log.INFO,
            "plugin_load",
            "sdk=${Build.VERSION.SDK_INT} package=${activity.packageName} " +
                "intentAction=${diagnosticValue(activity.intent?.action)}",
        )
        registerPermissionReceiver()
        installLifecycleCleanup()
        logAccessorySnapshot("plugin-load")
        processAccessoryIntent(activity.intent, announce = true)
    }

    override fun onNewIntent(intent: Intent) {
        logUsb(
            Log.INFO,
            "new_intent",
            "action=${diagnosticValue(intent.action)} hasAccessoryExtra=${accessoryFromIntent(intent) != null}",
        )
        logAccessorySnapshot("new-intent-before-processing")
        processAccessoryIntent(intent, announce = true)
    }

    @Command
    fun takeUsbAccessory(invoke: Invoke) {
        try {
            logAccessorySnapshot("take-command-start")
            val current = synchronized(lock) {
                val value = pending
                pending = null
                value
            }
            if (current != null) {
                logUsb(
                    Log.INFO,
                    "pending_accessory_claimed",
                    "fd=${current.fd} manufacturer=${diagnosticValue(current.manufacturer)} " +
                        "model=${diagnosticValue(current.model)} serialPresent=${current.serialNumber != null}",
                )
            } else {
                // A descriptor is claimable only after an actual
                // ACTION_USB_ACCESSORY_ATTACHED intent (or its permission result)
                // prepared it. UsbManager.accessoryList is deliberately not a
                // fallback: Pixel/Android can expose a stale accessory object
                // while the physical link is still MTP/ADB, and opening that
                // stale object monopolizes /dev/usb_accessory before the next
                // AOA START transition can complete.
                logUsb(Log.INFO, "take_command_no_pending_attachment", "available=false")
            }

            val result = JSObject()
            if (current == null) {
                result.put("available", false)
                result.put("fd", null)
                result.put("manufacturer", null)
                result.put("model", null)
                result.put("serialNumber", null)
                logUsb(Log.INFO, "take_command_result", "available=false")
            } else {
                result.put("available", true)
                result.put("fd", current.fd)
                result.put("manufacturer", current.manufacturer)
                result.put("model", current.model)
                result.put("serialNumber", current.serialNumber)
                logUsb(
                    Log.INFO,
                    "descriptor_handoff_to_rust",
                    "available=true fd=${current.fd} serialPresent=${current.serialNumber != null}",
                )
            }
            invoke.resolve(result)
        } catch (error: Exception) {
            logUsb(
                Log.ERROR,
                "take_command_failed",
                "error=${diagnosticValue(safeUsbMessage(error))}",
            )
            invoke.reject("The Android USB accessory could not be opened: ${safeUsbMessage(error)}")
        }
    }

    private fun processAccessoryIntent(intent: Intent?, announce: Boolean) {
        val action = intent?.action
        if (action != UsbManager.ACTION_USB_ACCESSORY_ATTACHED) {
            logUsb(
                Log.INFO,
                "accessory_intent_ignored",
                "action=${diagnosticValue(action)} reason=not-accessory-attach",
            )
            return
        }
        val accessory = intent?.let(::accessoryFromIntent)
        if (accessory == null) {
            logUsb(Log.WARN, "accessory_intent_missing_extra", "announce=$announce")
            return
        }
        if (!isOstudyAccessory(accessory)) {
            logUsb(
                Log.WARN,
                "accessory_intent_mismatch",
                "announce=$announce ${describeAccessory(accessory)}",
            )
            return
        }

        try {
            val hasPermission = manager.hasPermission(accessory)
            logUsb(
                Log.INFO,
                "accessory_intent_matched",
                "announce=$announce hasPermission=$hasPermission ${describeAccessory(accessory)}",
            )
            if (!hasPermission) {
                requestAccessoryPermission(manager, accessory)
                return
            }
            prepareAccessory(manager, accessory, announce)
        } catch (error: Exception) {
            logUsb(
                Log.ERROR,
                "accessory_intent_failed",
                "error=${diagnosticValue(safeUsbMessage(error))}",
            )
            Logger.error("Unable to prepare the oStudy USB accessory: ${safeUsbMessage(error)}")
        }
    }

    private fun requestAccessoryPermission(manager: UsbManager, accessory: UsbAccessory) {
        if (manager.hasPermission(accessory)) {
            logUsb(Log.INFO, "permission_already_granted", describeAccessory(accessory))
            prepareAccessory(manager, accessory, announce = true)
            return
        }
        val shouldRequest = synchronized(lock) {
            if (permissionRequestInFlight) {
                false
            } else {
                permissionRequestInFlight = true
                true
            }
        }
        if (!shouldRequest) {
            logUsb(Log.INFO, "permission_request_coalesced", describeAccessory(accessory))
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
            logUsb(
                Log.INFO,
                "permission_request_started",
                "sdk=${Build.VERSION.SDK_INT} action=$permissionAction ${describeAccessory(accessory)}",
            )
            Logger.info("Requesting Android permission for oStudy USB Sync accessory")
            manager.requestPermission(accessory, permissionIntent)
        } catch (error: Exception) {
            synchronized(lock) {
                permissionRequestInFlight = false
            }
            logUsb(
                Log.ERROR,
                "permission_request_failed",
                "error=${diagnosticValue(safeUsbMessage(error))}",
            )
            throw error
        }
    }

    private fun prepareAccessory(
        manager: UsbManager,
        accessory: UsbAccessory,
        announce: Boolean,
    ) {
        if (!manager.hasPermission(accessory)) {
            logUsb(Log.WARN, "prepare_without_permission", describeAccessory(accessory))
            return
        }
        logUsb(
            Log.INFO,
            "prepare_accessory_started",
            "announce=$announce ${describeAccessory(accessory)}",
        )
        val opened = openAccessory(manager, accessory)
        if (opened == null) {
            logUsb(Log.ERROR, "prepare_accessory_open_returned_null", describeAccessory(accessory))
            return
        }
        synchronized(lock) {
            pending?.close()
            pending = opened
        }
        logUsb(
            Log.INFO,
            "accessory_pending_for_rust",
            "fd=${opened.fd} announce=$announce serialPresent=${opened.serialNumber != null}",
        )
        if (announce) {
            val event = JSObject()
            event.put("manufacturer", opened.manufacturer)
            event.put("model", opened.model)
            event.put("serialNumber", opened.serialNumber)
            trigger("usb-sync-ready", event)
            logUsb(Log.INFO, "usb_sync_ready_emitted", "fd=${opened.fd}")
        }
    }

    private fun openAccessory(
        manager: UsbManager,
        accessory: UsbAccessory,
    ): PendingAccessory? {
        logUsb(Log.INFO, "open_accessory_started", describeAccessory(accessory))
        val descriptor = manager.openAccessory(accessory)
        if (descriptor == null) {
            logUsb(Log.ERROR, "open_accessory_failed", "reason=null-descriptor ${describeAccessory(accessory)}")
            return null
        }
        return try {
            val rawFd = descriptor.detachFd()
            val serial = accessory.serial
            logUsb(
                Log.INFO,
                "open_accessory_succeeded",
                "fd=$rawFd serialPresent=${serial != null}",
            )
            PendingAccessory(
                fd = rawFd,
                manufacturer = accessory.manufacturer ?: OSTUDY_ACCESSORY_MANUFACTURER,
                model = accessory.model ?: OSTUDY_ACCESSORY_MODEL,
                serialNumber = serial,
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
        logUsb(Log.INFO, "permission_receiver_registered", "action=$permissionAction")
    }

    private fun installLifecycleCleanup() {
        val owner = activity as? LifecycleOwner
        if (owner == null) {
            logUsb(Log.WARN, "lifecycle_observer_unavailable", "activity=${activity::class.java.simpleName}")
            return
        }
        val observer = object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                cleanup()
                owner.lifecycle.removeObserver(this)
            }
        }
        lifecycleObserver = observer
        owner.lifecycle.addObserver(observer)
        logUsb(Log.INFO, "lifecycle_observer_installed", "activity=${activity::class.java.simpleName}")
    }

    private fun cleanup() {
        logAccessorySnapshot("cleanup-start")
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
        logUsb(Log.INFO, "plugin_cleanup_complete")
    }

    private fun logAccessorySnapshot(stage: String) {
        val accessories = manager.accessoryList.orEmpty()
        val state = synchronized(lock) {
            "pending=${pending != null} permissionRequestInFlight=$permissionRequestInFlight"
        }
        logUsb(
            Log.INFO,
            "accessory_snapshot",
            "stage=$stage count=${accessories.size} $state",
        )
        accessories.forEachIndexed { index, accessory ->
            logUsb(
                Log.INFO,
                "accessory_seen",
                "stage=$stage index=$index hasPermission=${manager.hasPermission(accessory)} " +
                    describeAccessory(accessory),
            )
        }
    }

    private fun describeAccessory(accessory: UsbAccessory): String =
        "manufacturer=${diagnosticValue(accessory.manufacturer)} " +
            "model=${diagnosticValue(accessory.model)} " +
            "version=${diagnosticValue(accessory.version)} " +
            "uriPresent=${accessory.uri != null} matchesOstudy=${isOstudyAccessory(accessory)}"

    private fun logUsb(priority: Int, event: String, details: String = "") {
        val suffix = details.trim().takeIf(String::isNotEmpty)?.let { " $it" }.orEmpty()
        Log.println(priority, USB_DIAGNOSTIC_TAG, "event=$event$suffix")
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

private fun diagnosticValue(value: String?): String {
    val trimmed = value?.trim().orEmpty()
    if (trimmed.isEmpty()) return "null"
    return trimmed
        .replace(' ', '_')
        .replace('\t', '_')
        .replace('\n', '_')
        .replace('\r', '_')
        .take(96)
}

private fun safeUsbMessage(error: Throwable): String =
    error.message?.trim()?.takeIf { it.isNotEmpty() } ?: error::class.java.simpleName
