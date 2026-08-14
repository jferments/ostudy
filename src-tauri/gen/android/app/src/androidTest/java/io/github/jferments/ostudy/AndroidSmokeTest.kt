package io.github.jferments.ostudy

import android.app.Activity
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
@LargeTest
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class AndroidSmokeTest {
    @Test
    fun a0_appLaunchesAndTestBridgeIsReady() {
        withApplication { webView ->
            waitForBridge(webView)
            val bridgeType = decodeJavascriptString(
                evaluateSync(webView, "typeof window.__OSTUDY_ANDROID_TEST__")
            )
            assertEquals("object", bridgeType)
        }
    }

    @Test
    fun a_seedLibraryPracticeAndAnalytics() {
        withApplication { webView ->
            waitForBridge(webView)
            val result = evaluatePromise(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.seedAndReview()"
            )
            assertTrue(result.getInt("smokeCardCount") >= 1)
            assertTrue(result.getInt("recentReviewCount") >= 1)
            assertNotNull(result.optString("smokeDeckId").takeIf(String::isNotBlank))
        }
    }

    @Test
    fun b_restartPreservesLibraryAndReviewHistory() {
        withApplication { webView ->
            waitForBridge(webView)
            val result = evaluatePromise(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.snapshot()"
            )
            assertTrue(result.getInt("smokeCardCount") >= 1)
            assertTrue(result.getInt("recentReviewCount") >= 1)
        }
    }

    @Test
    fun c_documentsStubAndAndroidBackReturnToDecks() {
        withApplication { webView ->
            waitForBridge(webView)
            evaluateSync(
                webView,
                "document.querySelector('[data-testid=\"nav-more\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('[data-testid=\"nav-documents\"]'))"
                }
            )
            evaluateSync(
                webView,
                "document.querySelector('[data-testid=\"nav-documents\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('[data-testid=\"documents-development-stub\"]'))"
                }
            )
            waitForAndroidBackReady(webView)

            UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('h1')?.textContent?.includes('Decks'))"
                }
            )
        }
    }

    @Test
    fun c_nestedDeckAndEditorBackReturnThroughTheStack() {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            evaluateSync(
                webView,
                "document.querySelector('[data-testid=\"nav-decks\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Array.from(document.querySelectorAll('.deck-card-main')).some(button => button.textContent?.includes('Android Emulator Smoke Deck'))"
                }
            )
            evaluateSync(
                webView,
                "Array.from(document.querySelectorAll('.deck-card-main')).find(button => button.textContent?.includes('Android Emulator Smoke Deck'))?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('.deck-detail-header h1')?.textContent === 'Android Emulator Smoke Deck'"
                }
            )
            evaluateSync(
                webView,
                "Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === 'Add flashcard')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = { "Boolean(document.querySelector('.card-editor-screen'))" }
            )

            waitForAndroidBackReady(webView)
            device.pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = { "Boolean(document.querySelector('.deck-detail-header'))" }
            )
            waitForAndroidBackReady(webView)
            device.pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('.deck-grid')) && !document.querySelector('.deck-detail-header')"
                }
            )
        }
    }

    @Test
    fun d_reloadsIntoInterruptedPracticeSession() {
        withApplication { webView ->
            waitForBridge(webView)
            val started = evaluatePromise(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.beginInterruptedPractice()"
            )
            val sessionId = started.getString("sessionId")
            evaluateSync(
                webView,
                "delete window.__OSTUDY_ANDROID_TEST__; window.location.reload(); true"
            )
            waitForBridge(webView)
            val restored = evaluatePromise(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.restorationSnapshot()"
            )
            assertEquals(sessionId, restored.getString("sessionId"))
            assertTrue(restored.getBoolean("revealed"))
            assertFalse(restored.getBoolean("completed"))
        }
    }

    @Test
    fun e_processRestartKeepsInterruptedPracticeSession() {
        withApplication { webView ->
            waitForBridge(webView)
            val restored = evaluatePromise(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.restorationSnapshot()"
            )
            assertNotNull(restored.optString("sessionId").takeIf(String::isNotBlank))
            assertTrue(restored.getBoolean("revealed"))
            assertFalse(restored.getBoolean("completed"))
        }
    }

    @Test
    fun f_studyContentImportPickerCancelsCleanly() {
        assertImportPickerCancels("study_content", "Android study-content import picker")
    }

    @Test
    fun f_imageImportPickerCancelsCleanly() {
        assertImportPickerCancels("image", "Android image import picker")
    }

    @Test
    fun f_pdfImportPickerCancelsCleanly() {
        assertImportPickerCancels("pdf_document", "Android PDF import picker")
    }

    @Test
    fun f_libraryBackupImportPickerCancelsCleanly() {
        assertImportPickerCancels("library_backup", "Android library-backup import picker")
    }

    @Test
    fun f_deckExportPickerCancelsCleanly() {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            cancelFileOperation(
                webView,
                device,
                "window.__OSTUDY_ANDROID_TEST__.startDeckExportPicker()",
                timeoutMillis = 30_000,
                label = "Android deck export picker"
            )
        }
    }

    @Test
    fun g_sequentialPickerCancellationReleasesNativeOperationSlot() {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            for ((purpose, label) in listOf(
                "study_content" to "Android study-content import picker",
                "image" to "Android image import picker",
                "pdf_document" to "Android PDF import picker",
                "library_backup" to "Android library-backup import picker"
            )) {
                cancelFileOperation(
                    webView,
                    device,
                    "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')",
                    timeoutMillis = 20_000,
                    label = label
                )
            }
            cancelFileOperation(
                webView,
                device,
                "window.__OSTUDY_ANDROID_TEST__.startDeckExportPicker()",
                timeoutMillis = 30_000,
                label = "Android deck export picker"
            )
        }
    }

    private fun assertImportPickerCancels(purpose: String, label: String) {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            cancelFileOperation(
                webView,
                device,
                "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')",
                timeoutMillis = 20_000,
                label = label
            )
        }
    }

    private fun cancelFileOperation(
        webView: WebView,
        device: UiDevice,
        expression: String,
        timeoutMillis: Long,
        label: String
    ) {
        val sequence = startFileOperation(webView, expression)
        waitForDocumentsUiOrFailure(webView, device, sequence, timeoutMillis, label)
        device.pressBack()
        waitForDocumentsUiClosed(device)
        waitForBridge(webView)
        waitForFileOperationSettled(webView, sequence, label)
    }

    private fun startFileOperation(webView: WebView, expression: String): Int {
        val snapshot = evaluateJsonObject(webView, expression)
        assertEquals(snapshot.optString("error"), "pending", snapshot.getString("phase"))
        return snapshot.getInt("sequence")
    }

    private fun waitForDocumentsUiOrFailure(
        webView: WebView,
        device: UiDevice,
        sequence: Int,
        timeoutMillis: Long,
        label: String
    ) {
        val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMillis)
        var latest = fileOperationSnapshot(webView)
        while (System.nanoTime() < deadline) {
            if (documentsUiVisible(device)) return
            latest = fileOperationSnapshot(webView)
            if (latest.optInt("sequence") == sequence && latest.optString("phase") == "error") {
                throw AssertionError(
                    "$label failed before opening: ${latest.optString("error", "unknown error")}"
                )
            }
            Thread.sleep(100)
        }
        throw AssertionError(
            "$label did not open. operationPhase=${latest.optString("phase")} " +
                "operationError=${latest.optString("error", "none")}"
        )
    }

    private fun waitForFileOperationSettled(
        webView: WebView,
        sequence: Int,
        label: String
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20)
        var latest = fileOperationSnapshot(webView)
        while (System.nanoTime() < deadline) {
            latest = fileOperationSnapshot(webView)
            if (latest.optInt("sequence") == sequence) {
                when (latest.optString("phase")) {
                    "settled" -> return
                    "error" -> throw AssertionError(
                        "$label failed after picker result: " +
                            latest.optString("error", "unknown error")
                    )
                }
            }
            Thread.sleep(100)
        }
        throw AssertionError(
            "$label did not settle after the picker closed. " +
                "operationPhase=${latest.optString("phase")} " +
                "operationError=${latest.optString("error", "none")}"
        )
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

    private fun documentsUiVisible(device: UiDevice): Boolean =
        device.hasObject(By.pkg("com.google.android.documentsui")) ||
            device.hasObject(By.pkg("com.android.documentsui"))

    private fun waitForDocumentsUiClosed(device: UiDevice) {
        device.wait(Until.gone(By.pkg("com.google.android.documentsui")), 5_000)
        device.wait(Until.gone(By.pkg("com.android.documentsui")), 5_000)
        assertFalse("Android DocumentsUI did not close", documentsUiVisible(device))
    }

    private fun waitForAndroidBackReady(webView: WebView) {
        waitUntil(
            webView,
            predicate = { it == "true" },
            expression = {
                "document.documentElement.dataset.androidBackInterception === 'true' && document.documentElement.dataset.overlayActive !== 'true'"
            }
        )
    }

    private fun withApplication(block: (WebView) -> Unit) {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        var webView: WebView? = null
        scenario.onActivity { activity ->
            webView = findWebView(activity)
        }
        val target = requireNotNull(webView) { "The Tauri WebView was not found." }
        block(target)
    }

    private fun waitForBridge(webView: WebView) {
        waitUntil(
            webView,
            timeoutSeconds = 30,
            predicate = { it == "true" },
            expression = { "typeof window.__OSTUDY_ANDROID_TEST__ === 'object'" }
        )
    }

    private fun evaluatePromise(webView: WebView, expression: String): JSONObject {
        val key = "__ostudySmoke${UUID.randomUUID().toString().replace("-", "")}"
        evaluateSync(
            webView,
            """
            (() => {
              window['$key'] = null;
              Promise.resolve($expression)
                .then(value => { window['$key'] = JSON.stringify({ ok: true, value }); })
                .catch(error => { window['$key'] = JSON.stringify({ ok: false, error: String(error) }); });
              return true;
            })()
            """.trimIndent()
        )
        val envelope = waitUntil(
            webView,
            timeoutSeconds = 45,
            predicate = { it != "null" },
            expression = { "window['$key']" }
        )
        val parsed = JSONObject(decodeJavascriptString(envelope))
        assertEquals(parsed.optString("error"), true, parsed.getBoolean("ok"))
        return parsed.getJSONObject("value")
    }

    private fun waitUntil(
        webView: WebView,
        timeoutSeconds: Long = 15,
        predicate: (String) -> Boolean,
        expression: () -> String
    ): String {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(timeoutSeconds)
        var latest = "null"
        while (System.nanoTime() < deadline) {
            latest = evaluateSync(webView, expression())
            if (predicate(latest)) return latest
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for JavaScript expression. Last result: $latest")
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
}
