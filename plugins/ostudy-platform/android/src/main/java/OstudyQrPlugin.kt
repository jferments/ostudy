package io.github.jferments.ostudy.platform

import android.app.Activity
import android.util.Log
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import java.util.concurrent.atomic.AtomicBoolean

private const val LOG_TAG = "ostudy.qr"
private const val MAX_QR_CONTENT_CHARS = 16 * 1024
private const val MAX_PROMPT_CHARS = 240

@InvokeArg
class ScanQrCodeArgs {
    var prompt: String = ""
}

@TauriPlugin
class OstudyQrPlugin(private val activity: Activity) : Plugin(activity) {
    private val scanOpen = AtomicBoolean(false)
    private val contract = ScanContract()

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
        val args = invoke.parseArgs(ScanQrCodeArgs::class.java)
        try {
            val prompt = args.prompt
                .filter { !it.isISOControl() || it == ' ' }
                .take(MAX_PROMPT_CHARS)
                .ifBlank { "Scan the oStudy pairing QR code" }
            val options = ScanOptions()
                .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                .setPrompt(prompt)
                .setOrientationLocked(false)
                .setBeepEnabled(false)
                .setBarcodeImageEnabled(false)
            Log.d(LOG_TAG, "event=qr_scan_opened backend=zxing_embedded")
            startActivityForResult(invoke, contract.createIntent(activity, options), "scanQrCodeResult")
        } catch (error: Exception) {
            scanOpen.set(false)
            Log.w(
                LOG_TAG,
                "event=qr_scan_failed phase=open error_type=${error::class.java.simpleName}",
            )
            invoke.reject("The local QR scanner could not open: ${safeMessage(error)}")
        }
    }

    @ActivityCallback
    private fun scanQrCodeResult(invoke: Invoke, result: ActivityResult) {
        if (!scanOpen.compareAndSet(true, false)) {
            invoke.reject("The QR scan is no longer active.")
            return
        }
        try {
            val parsed = contract.parseResult(result.resultCode, result.data)
            val contents = parsed.contents
            when {
                result.resultCode == Activity.RESULT_CANCELED || contents == null -> {
                    Log.d(LOG_TAG, "event=qr_scan_cancelled")
                    invoke.resolve(JSObject().apply {
                        put("cancelled", true)
                        put("contents", null)
                    })
                }
                parsed.formatName != ScanOptions.QR_CODE -> {
                    Log.w(LOG_TAG, "event=qr_scan_rejected reason=unexpected_format")
                    invoke.reject("The scanner returned a non-QR barcode result.")
                }
                contents.length > MAX_QR_CONTENT_CHARS -> {
                    Log.w(LOG_TAG, "event=qr_scan_rejected reason=content_too_large")
                    invoke.reject("The scanned QR code exceeds oStudy's pairing-code size limit.")
                }
                else -> {
                    Log.d(LOG_TAG, "event=qr_scan_completed content_length=${contents.length}")
                    invoke.resolve(JSObject().apply {
                        put("cancelled", false)
                        put("contents", contents)
                    })
                }
            }
        } catch (error: Exception) {
            Log.w(
                LOG_TAG,
                "event=qr_scan_failed phase=result error_type=${error::class.java.simpleName}",
            )
            invoke.reject("The local QR scanner could not complete: ${safeMessage(error)}")
        }
    }

    private fun safeMessage(error: Throwable): String =
        (error.message ?: error::class.java.simpleName)
            .filter { !it.isISOControl() || it == ' ' }
            .take(240)
}
