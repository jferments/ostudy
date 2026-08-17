package io.github.jferments.ostudy.platform

import android.app.Activity
import android.util.Log
import android.webkit.WebView
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import java.util.concurrent.atomic.AtomicBoolean

private const val LOG_TAG = "ostudy.qr"
private const val MAX_QR_CONTENT_CHARS = 16 * 1024

/**
 * The prompt field remains in the mobile-plugin contract so existing Rust and
 * frontend callers do not need a platform-specific branch. Google Code Scanner
 * owns its permissionless native UI and does not expose custom prompt text.
 */
@InvokeArg
class ScanQrCodeArgs {
    var prompt: String = ""
}

@TauriPlugin
class OstudyQrPlugin(private val activity: Activity) : Plugin(activity) {
    private val scanOpen = AtomicBoolean(false)
    private val options = GmsBarcodeScannerOptions.Builder()
        .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
        .enableAutoZoom()
        .build()

    override fun load(webView: WebView) {
        val owner = activity as? LifecycleOwner ?: return
        owner.lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                scanOpen.set(false)
                owner.lifecycle.removeObserver(this)
            }
        })
    }

    @Command
    fun scanQrCode(invoke: Invoke) {
        if (!scanOpen.compareAndSet(false, true)) {
            invoke.reject("Another QR-code scanner is already open.")
            return
        }
        // Validate the existing IPC shape even though the first-party scanner
        // controls its own native instructions.
        invoke.parseArgs(ScanQrCodeArgs::class.java)
        val completed = AtomicBoolean(false)

        fun complete(deliver: () -> Unit) {
            if (!completed.compareAndSet(false, true)) return
            scanOpen.set(false)
            deliverWhenResumed(deliver)
        }

        activity.runOnUiThread {
            try {
                Log.d(LOG_TAG, "event=qr_scan_opened backend=google_code_scanner")
                GmsBarcodeScanning.getClient(activity, options)
                    .startScan()
                    .addOnSuccessListener { barcode ->
                        val contents = barcode.rawValue
                        when {
                            contents == null -> complete {
                                Log.w(LOG_TAG, "event=qr_scan_failed reason=missing_raw_value")
                                invoke.reject("The scanned QR code did not contain readable text.")
                            }
                            contents.length > MAX_QR_CONTENT_CHARS -> complete {
                                Log.w(LOG_TAG, "event=qr_scan_rejected reason=content_too_large")
                                invoke.reject(
                                    "The scanned QR code exceeds ostudy's pairing-code size limit.",
                                )
                            }
                            else -> complete {
                                Log.d(
                                    LOG_TAG,
                                    "event=qr_scan_completed content_length=${contents.length}",
                                )
                                invoke.resolve(JSObject().apply {
                                    put("cancelled", false)
                                    put("contents", contents)
                                })
                            }
                        }
                    }
                    .addOnCanceledListener {
                        complete {
                            Log.d(LOG_TAG, "event=qr_scan_cancelled")
                            invoke.resolve(JSObject().apply {
                                put("cancelled", true)
                                put("contents", null)
                            })
                        }
                    }
                    .addOnFailureListener { error ->
                        complete {
                            Log.w(
                                LOG_TAG,
                                "event=qr_scan_failed error_type=${error::class.java.simpleName}",
                            )
                            invoke.reject(
                                "Google Code Scanner could not complete the scan: ${safeMessage(error)}",
                            )
                        }
                    }
            } catch (error: Exception) {
                complete {
                    Log.w(
                        LOG_TAG,
                        "event=qr_scan_failed phase=open error_type=${error::class.java.simpleName}",
                    )
                    invoke.reject(
                        "Google Code Scanner could not open: ${safeMessage(error)}",
                    )
                }
            }
        }
    }

    private fun deliverWhenResumed(deliver: () -> Unit) {
        val owner = activity as? LifecycleOwner
        if (owner == null) {
            activity.runOnUiThread(deliver)
            return
        }
        activity.runOnUiThread {
            if (owner.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                deliver()
                return@runOnUiThread
            }
            if (owner.lifecycle.currentState == Lifecycle.State.DESTROYED) return@runOnUiThread
            val observer = object : DefaultLifecycleObserver {
                override fun onResume(owner: LifecycleOwner) {
                    owner.lifecycle.removeObserver(this)
                    deliver()
                }

                override fun onDestroy(owner: LifecycleOwner) {
                    owner.lifecycle.removeObserver(this)
                }
            }
            owner.lifecycle.addObserver(observer)
        }
    }

    private fun safeMessage(error: Throwable): String =
        (error.message ?: error::class.java.simpleName)
            .filter { !it.isISOControl() || it == ' ' }
            .take(240)
}
