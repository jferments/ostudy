package io.github.jferments.ostudy

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.SystemClock
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import app.tauri.plugin.PluginHandle
import app.tauri.plugin.PluginManager
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Diagnostic-only coverage for the intermittent Android document-picker cancellation race.
 *
 * This class intentionally does not alter the production plugin or the authoritative smoke
 * contract. When JavaScript remains pending after DocumentsUI closes, reflection into Tauri's
 * in-process PluginManager tells us whether OstudyPlatformPlugin.pendingImport was cleared.
 * That distinguishes a missing ActivityResult callback from a later invoke/JNI/WebView delivery
 * failure without changing application behavior to make the test pass.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidPickerCancellationDiagnosticTest {
    @Test
    fun pickerCancellationBoundary() {
        val purpose =
            InstrumentationRegistry.getArguments().getString("pickerPurpose") ?: "study_content"
        require(purpose in supportedPurposes) {
            "Unsupported pickerPurpose=$purpose; expected one of $supportedPurposes"
        }

        val scenario = ActivityScenario.launch(MainActivity::class.java)
        try {
            val webView = waitForWebView(scenario)
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            val pickerPackages = documentPickerPackages(Intent.ACTION_OPEN_DOCUMENT)
            val startedAt = SystemClock.elapsedRealtime()

            val initial = evaluateJsonObject(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')"
            )
            assertEquals(initial.optString("error"), "pending", initial.getString("phase"))
            val sequence = initial.getInt("sequence")
            trace(
                purpose,
                sequence,
                "operation-started",
                startedAt,
                "nativePendingImport=${nativePendingImportState()}"
            )

            waitForPickerVisible(device, pickerPackages, purpose, sequence, startedAt)
            trace(
                purpose,
                sequence,
                "press-back",
                startedAt,
                "currentPackage=${device.currentPackageName}"
            )
            assertTrue("Android Back was not dispatched to the picker", device.pressBack())
            waitForPickerClosed(device, pickerPackages, purpose, sequence, startedAt)
            waitForBridge(webView)
            waitForCancellationSettlement(webView, purpose, sequence, startedAt)
        } finally {
            scenario.close()
        }
    }

    private fun waitForPickerVisible(
        device: UiDevice,
        pickerPackages: Set<String>,
        purpose: String,
        sequence: Int,
        startedAt: Long
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20)
        while (System.nanoTime() < deadline) {
            if (pickerVisible(device, pickerPackages)) {
                trace(
                    purpose,
                    sequence,
                    "picker-visible",
                    startedAt,
                    "currentPackage=${device.currentPackageName} nativePendingImport=${nativePendingImportState()}"
                )
                return
            }
            Thread.sleep(100)
        }
        throw AssertionError(
            "Picker did not become visible. purpose=$purpose packages=$pickerPackages " +
                "nativePendingImport=${nativePendingImportState()}"
        )
    }

    private fun waitForPickerClosed(
        device: UiDevice,
        pickerPackages: Set<String>,
        purpose: String,
        sequence: Int,
        startedAt: Long
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (System.nanoTime() < deadline) {
            if (!pickerVisible(device, pickerPackages)) {
                trace(
                    purpose,
                    sequence,
                    "picker-closed",
                    startedAt,
                    "currentPackage=${device.currentPackageName} nativePendingImport=${nativePendingImportState()}"
                )
                return
            }
            Thread.sleep(100)
        }
        throw AssertionError(
            "Picker did not close after Android Back. purpose=$purpose " +
                "nativePendingImport=${nativePendingImportState()}"
        )
    }

    private fun waitForCancellationSettlement(
        webView: WebView,
        purpose: String,
        sequence: Int,
        startedAt: Long
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20)
        var latest = fileOperationSnapshot(webView)
        var lastTraceBucket = -1L

        while (System.nanoTime() < deadline) {
            latest = fileOperationSnapshot(webView)
            val elapsed = SystemClock.elapsedRealtime() - startedAt
            val traceBucket = elapsed / 1_000
            if (traceBucket != lastTraceBucket && traceBucket in setOf(1L, 2L, 5L, 10L, 15L)) {
                lastTraceBucket = traceBucket
                trace(
                    purpose,
                    sequence,
                    "settlement-wait",
                    startedAt,
                    "jsPhase=${latest.optString("phase")} " +
                        "jsError=${latest.optString("error", "none")} " +
                        "nativePendingImport=${nativePendingImportState()}"
                )
            }

            if (latest.optInt("sequence") == sequence) {
                when (latest.optString("phase")) {
                    "settled" -> {
                        assertTrue(
                            "Picker resolved without reporting cancellation: $latest",
                            latest.getBoolean("cancelled")
                        )
                        trace(
                            purpose,
                            sequence,
                            "settled-cancelled",
                            startedAt,
                            "nativePendingImport=${nativePendingImportState()}"
                        )
                        return
                    }
                    "error" -> throw AssertionError(
                        "Picker failed after result. purpose=$purpose snapshot=$latest " +
                            "nativePendingImport=${nativePendingImportState()}"
                    )
                }
            }
            Thread.sleep(100)
        }

        val nativePending = nativePendingImportState()
        trace(
            purpose,
            sequence,
            "settlement-timeout",
            startedAt,
            "jsPhase=${latest.optString("phase")} jsError=${latest.optString("error", "none")} " +
                "nativePendingImport=$nativePending"
        )
        throw AssertionError(
            "Picker cancellation did not settle. purpose=$purpose " +
                "operationPhase=${latest.optString("phase")} " +
                "operationError=${latest.optString("error", "none")} " +
                "nativePendingImport=$nativePending. " +
                "If nativePendingImport=true, the plugin ActivityResult callback did not clear its slot; " +
                "if false, the callback ran and completion was lost after that boundary."
        )
    }

    private fun nativePendingImportState(): String {
        return try {
            val pluginsField = PluginManager::class.java.getDeclaredField("plugins")
            pluginsField.isAccessible = true
            @Suppress("UNCHECKED_CAST")
            val plugins = pluginsField.get(PluginManager) as Map<String, PluginHandle>
            val platformHandle = plugins.values.firstOrNull {
                it.instance.javaClass.name.endsWith(".OstudyPlatformPlugin")
            } ?: return "plugin-not-found"

            val pendingImportField =
                platformHandle.instance.javaClass.getDeclaredField("pendingImport")
            pendingImportField.isAccessible = true
            if (pendingImportField.get(platformHandle.instance) == null) "false" else "true"
        } catch (error: ReflectiveOperationException) {
            "reflection-error:${error.javaClass.simpleName}"
        }
    }

    private fun documentPickerPackages(action: String): Set<String> {
        val packageManager =
            InstrumentationRegistry.getInstrumentation().targetContext.packageManager
        val intent = Intent(action).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
        }
        val resolved = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            packageManager.queryIntentActivities(
                intent,
                PackageManager.ResolveInfoFlags.of(PackageManager.MATCH_DEFAULT_ONLY.toLong())
            )
        } else {
            @Suppress("DEPRECATION")
            packageManager.queryIntentActivities(intent, PackageManager.MATCH_DEFAULT_ONLY)
        }
        val packages = linkedSetOf<String>()
        resolved.mapNotNullTo(packages) { it.activityInfo?.packageName }
        packages.add("com.google.android.documentsui")
        packages.add("com.android.documentsui")
        return packages
    }

    private fun pickerVisible(device: UiDevice, pickerPackages: Set<String>): Boolean {
        val current = device.currentPackageName
        if (current != null && current in pickerPackages) return true
        return pickerPackages.any { device.hasObject(By.pkg(it)) }
    }

    private fun fileOperationSnapshot(webView: WebView): JSONObject =
        evaluateJsonObject(
            webView,
            "window.__OSTUDY_ANDROID_TEST__.fileOperationSnapshot()"
        )

    private fun evaluateJsonObject(webView: WebView, expression: String): JSONObject {
        val encoded = evaluateSync(webView, "JSON.stringify($expression)")
        return JSONObject(decodeJavascriptString(encoded))
    }

    private fun waitForBridge(webView: WebView) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var latest = "unavailable"
        while (System.nanoTime() < deadline) {
            latest = decodeJavascriptString(
                evaluateSync(webView, "typeof window.__OSTUDY_ANDROID_TEST__")
            )
            if (latest == "object") return
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for Android smoke bridge. Last type=$latest")
    }

    private fun waitForWebView(scenario: ActivityScenario<MainActivity>): WebView {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15)
        var webView: WebView? = null
        while (System.nanoTime() < deadline) {
            scenario.onActivity { activity -> webView = findWebView(activity) }
            webView?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("Timed out waiting for the Tauri WebView to attach")
    }

    private fun evaluateSync(webView: WebView, script: String): String {
        val latch = CountDownLatch(1)
        var result = "null"
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            webView.evaluateJavascript(script) { value ->
                result = value ?: "null"
                latch.countDown()
            }
        }
        assertTrue("JavaScript evaluation timed out", latch.await(15, TimeUnit.SECONDS))
        return result
    }

    private fun decodeJavascriptString(value: String): String {
        if (value == "null") return value
        return JSONArray("[$value]").getString(0)
    }

    private fun findWebView(activity: Activity): WebView? =
        findWebView(activity.window.decorView)

    private fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) {
            for (index in 0 until view.childCount) {
                findWebView(view.getChildAt(index))?.let { return it }
            }
        }
        return null
    }

    private fun trace(
        purpose: String,
        sequence: Int,
        phase: String,
        startedAt: Long,
        details: String
    ) {
        Log.i(
            logTag,
            "purpose=$purpose sequence=$sequence phase=$phase " +
                "elapsedMs=${SystemClock.elapsedRealtime() - startedAt} $details"
        )
    }

    private companion object {
        const val logTag = "OstudyPickerDiag"
        val supportedPurposes =
            setOf("study_content", "image", "pdf_document", "library_backup")
    }
}
