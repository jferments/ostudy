package io.github.jferments.ostudy.platform

import android.app.Activity
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.zxing.BarcodeFormat
import com.google.zxing.integration.android.IntentIntegrator

private const val MAX_QR_CONTENT_CHARS = 16 * 1024

@InvokeArg
class ScanQrCodeArgs {
    var prompt: String = "Scan an ostudy pairing code"
}

@TauriPlugin
class OstudyQrPlugin(private val activity: Activity) : Plugin(activity) {
    @Volatile private var scanOpen = false

    override fun load(webView: WebView) {
        val owner = activity as? LifecycleOwner ?: return
        owner.lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                scanOpen = false
                owner.lifecycle.removeObserver(this)
            }
        })
    }

    @Command
    fun scanQrCode(invoke: Invoke) {
        if (scanOpen) {
            invoke.reject("Another QR-code scanner is already open.")
            return
        }
        val args = invoke.parseArgs(ScanQrCodeArgs::class.java)
        try {
            scanOpen = true
            val intent = IntentIntegrator(activity)
                .setDesiredBarcodeFormats(listOf(BarcodeFormat.QR_CODE.name))
                .setPrompt(args.prompt.take(160).ifBlank { "Scan an ostudy pairing code" })
                .setBeepEnabled(false)
                .setBarcodeImageEnabled(false)
                .setOrientationLocked(false)
                .createScanIntent()
            startActivityForResult(invoke, intent, "scanQrCodeResult")
        } catch (error: Exception) {
            scanOpen = false
            invoke.reject("The Android QR scanner could not open: ${safeMessage(error)}")
        }
    }

    @ActivityCallback
    fun scanQrCodeResult(invoke: Invoke, result: ActivityResult) {
        scanOpen = false
        val parsed = IntentIntegrator.parseActivityResult(result.resultCode, result.data)
        if (result.resultCode == Activity.RESULT_CANCELED || parsed.contents == null) {
            deliverWhenResumed {
                invoke.resolve(JSObject().apply {
                    put("cancelled", true)
                    put("contents", null)
                })
            }
            return
        }
        val contents = parsed.contents
        if (contents.length > MAX_QR_CONTENT_CHARS) {
            deliverWhenResumed {
                invoke.reject("The scanned QR code exceeds ostudy's pairing-code size limit.")
            }
            return
        }
        deliverWhenResumed {
            invoke.resolve(JSObject().apply {
                put("cancelled", false)
                put("contents", contents)
            })
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
