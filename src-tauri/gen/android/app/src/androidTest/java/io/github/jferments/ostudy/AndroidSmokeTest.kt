package io.github.jferments.ostudy

import android.app.Activity
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.UiDevice
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
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

    private fun withApplication(block: (WebView) -> Unit) {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            var webView: WebView? = null
            scenario.onActivity { activity ->
                webView = findWebView(activity)
            }
            val target = requireNotNull(webView) { "The Tauri WebView was not found." }
            block(target)
        }
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
