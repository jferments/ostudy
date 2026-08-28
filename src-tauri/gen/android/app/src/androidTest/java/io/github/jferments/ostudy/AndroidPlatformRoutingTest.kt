package io.github.jferments.ostudy

import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidPlatformRoutingTest {
    @Test
    fun rustPlatformSnapshotReachesDocumentAndQrNativeRoutes() {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        val webView = waitForWebView(scenario)
        waitForBridge(webView)
        val health = evaluatePromise(
            webView,
            "window.__OSTUDY_ANDROID_TEST__.platformHealth()"
        )
        assertEquals("mobile", health.getString("runtimeFamily"))
        assertEquals("unavailable", health.getString("windowMaximizeRestore"))
        assertEquals("unavailable", health.getString("windowFullScreen"))
        assertEquals("available", health.getString("nativePdfRender"))
        assertEquals("available", health.getString("ocr"))
        assertEquals("available", health.getString("qrScan"))
    }

    private fun waitForWebView(scenario: ActivityScenario<MainActivity>): WebView {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15)
        var webView: WebView? = null
        while (System.nanoTime() < deadline) {
            scenario.onActivity { activity ->
                webView = findWebView(activity.window.decorView)
            }
            webView?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError("Timed out waiting for the Tauri WebView to attach.")
    }

    private fun waitForBridge(webView: WebView) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var latest = "null"
        while (System.nanoTime() < deadline) {
            latest = evaluateSync(
                webView,
                "JSON.stringify({type: typeof window.__OSTUDY_ANDROID_TEST__, state: document.documentElement?.dataset?.androidTestBridge || 'unset', error: document.documentElement?.dataset?.androidTestBridgeError || ''})"
            )
            val decoded = decodeJavascriptString(latest)
            if (decoded != "null") {
                val snapshot = JSONObject(decoded)
                if (snapshot.optString("type") == "object") return
                if (snapshot.optString("state") in setOf("missing", "error")) {
                    throw AssertionError("Android smoke bridge initialization failed: $snapshot")
                }
            }
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for Android smoke bridge. Last result: $latest")
    }

    private fun evaluatePromise(webView: WebView, expression: String): JSONObject {
        val key = "__ostudyRouting${UUID.randomUUID().toString().replace("-", "")}"
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
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(45)
        var envelope = "null"
        while (System.nanoTime() < deadline) {
            envelope = evaluateSync(webView, "window['$key']")
            if (envelope != "null") break
            Thread.sleep(100)
        }
        if (envelope == "null") throw AssertionError("Native platform health check timed out.")
        val parsed = JSONObject(decodeJavascriptString(envelope))
        assertTrue(parsed.optString("error"), parsed.getBoolean("ok"))
        return parsed.getJSONObject("value")
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
