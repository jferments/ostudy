package io.github.jferments.ostudy.platform

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.hardware.usb.UsbAccessory
import android.hardware.usb.UsbManager
import android.os.Build
import android.os.ParcelFileDescriptor
import android.webkit.WebView
import app.tauri.Logger
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val OSTUDY_ACCESSORY_MANUFACTURER = "oStudy"
private const val OSTUDY_ACCESSORY_MODEL = "oStudy USB Sync"

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
    private var pending: PendingAccessory? = null

    override fun load(webView: WebView) {
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
                val manager = activity.getSystemService(Context.USB_SERVICE) as UsbManager
                val accessory = manager.accessoryList
                    ?.firstOrNull(::isOstudyAccessory)
                if (accessory != null && manager.hasPermission(accessory)) {
                    current = openAccessory(manager, accessory)
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
            val manager = activity.getSystemService(Context.USB_SERVICE) as UsbManager
            if (!manager.hasPermission(accessory)) {
                Logger.warn("oStudy USB accessory was attached without Android accessory permission")
                return
            }
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
        } catch (error: Exception) {
            Logger.error("Unable to prepare the oStudy USB accessory: ${safeUsbMessage(error)}")
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
