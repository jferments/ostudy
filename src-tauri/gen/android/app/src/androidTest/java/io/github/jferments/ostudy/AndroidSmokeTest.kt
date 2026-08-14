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
            evaluateSync(webView, "window.location.reload(); true")
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
    fun f_systemImportImageAndExportPickersCancelCleanly() {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())

            evaluateSync(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.openImportPicker('study_content'); true"
            )
            assertTrue(
                "Android document picker did not open for deck import",
                waitForDocumentsUi(device, 10_000)
            )
            device.pressBack()
            waitForDocumentsUiClosed(device)
            waitForBridge(webView)

            evaluateSync(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.openImportPicker('image'); true"
            )
            assertTrue(
                "Android document picker did not open for image import",
                waitForDocumentsUi(device, 10_000)
            )
            device.pressBack()
            waitForDocumentsUiClosed(device)
            waitForBridge(webView)

            evaluateSync(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.openDeckExportPicker(); true"
            )
            assertTrue(
                "Android create-document picker did not open for deck export",
                waitForDocumentsUi(device, 15_000)
            )
            device.pressBack()
            waitForDocumentsUiClosed(device)
            waitForBridge(webView)
        }
    }

    private fun waitForDocumentsUi(device: UiDevice, timeoutMillis: Long): Boolean =
        device.wait(Until.hasObject(By.pkg("com.google.android.documentsui")), timeoutMillis) ||
            device.wait(Until.hasObject(By.pkg("com.android.documentsui")), 1_000)

    private fun waitForDocumentsUiClosed(device: UiDevice) {
        device.wait(Until.gone(By.pkg("com.google.android.documentsui")), 5_000)
        device.wait(Until.gone(By.pkg("com.android.documentsui")), 5_000)
        // DocumentsUI can disappear one main-loop turn before ActivityResult is
        // delivered back to Tauri. Give the callback time to clear the native
        // pending-operation slot before opening the next picker.
        Thread.sleep(250)
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
        // Do not close ActivityScenario from inside the instrumentation process.
        // Destroying the only Tauri Activity is an application lifecycle event
        // and may tear down the Android event loop before AndroidJUnitRunner can
        // report the test result. The shell smoke runner owns deterministic
        // process cleanup with `am force-stop` between cases instead.
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
