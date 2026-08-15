package io.github.jferments.ostudy

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
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
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidDocumentsProviderSmokeTest {
    @Test
    fun h_documentProviderRoundTripStreamsImportsAndExport() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val device = UiDevice.getInstance(instrumentation)
        OstudyTestDocumentsProvider.resetExports(instrumentation.context)

        withApplication { webView ->
            waitForBridge(webView)
            selectFixture(
                webView,
                device,
                purpose = "study_content",
                fixture = OstudyTestDocumentsProvider.JSON_FIXTURE,
                expectedKind = "generated_json"
            )
            selectFixture(
                webView,
                device,
                purpose = "image",
                fixture = OstudyTestDocumentsProvider.IMAGE_FIXTURE,
                expectedKind = "png"
            )
            selectFixture(
                webView,
                device,
                purpose = "pdf_document",
                fixture = OstudyTestDocumentsProvider.PDF_FIXTURE,
                expectedKind = "pdf"
            )

            val exportStart = evaluateJsonObject(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.startDeckExportPicker()"
            )
            assertEquals("pending", exportStart.getString("phase"))
            val exportSequence = exportStart.getInt("sequence")
            val pickerPackages = documentPickerPackages(Intent.ACTION_CREATE_DOCUMENT)
            waitForPicker(device, pickerPackages, "Android deck export picker")
            openFixtureRoot(device)
            pressSave(device, pickerPackages)
            waitForPickerClosed(device, pickerPackages, "Android deck export picker")
            waitForBridge(webView)
            val exportResult = waitForSettledOperation(webView, exportSequence, "Android deck export")
            assertFalse(exportResult.getBoolean("cancelled"))
            val result = exportResult.getJSONObject("result")
            assertEquals("android_document_provider", result.getString("destinationKind"))
            assertTrue(result.getLong("byteSize") > 0L)

            val exported = waitForExportedFile(instrumentation.context)
            assertTrue(exported.length() > 0L)
            assertEquals(result.getLong("byteSize"), exported.length())
        }
    }

    private fun selectFixture(
        webView: WebView,
        device: UiDevice,
        purpose: String,
        fixture: String,
        expectedKind: String
    ) {
        val start = evaluateJsonObject(
            webView,
            "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')"
        )
        assertEquals("pending", start.getString("phase"))
        val sequence = start.getInt("sequence")
        val pickerPackages = documentPickerPackages(Intent.ACTION_OPEN_DOCUMENT)
        waitForPicker(device, pickerPackages, "Android $purpose import picker")
        openFixtureRoot(device)
        val fixtureObject = device.wait(Until.findObject(By.text(fixture)), 10_000)
        assertNotNull("System picker did not display $fixture", fixtureObject)
        fixtureObject.click()
        waitForPickerClosed(device, pickerPackages, "Android $purpose import picker")
        waitForBridge(webView)

        val settled = waitForSettledOperation(webView, sequence, "Android $purpose import")
        assertFalse(settled.getBoolean("cancelled"))
        val result = settled.getJSONObject("result")
        assertEquals(fixture, result.getString("displayName"))
        assertEquals(expectedKind, result.getString("detectedKind"))
        assertTrue(result.getLong("byteSize") > 0L)
    }

    private fun openFixtureRoot(device: UiDevice) {
        device.findObject(By.text(OstudyTestDocumentsProvider.ROOT_TITLE))?.let {
            it.click()
            return
        }

        val rootButton =
            device.findObject(By.desc("Show roots"))
                ?: device.findObject(By.descContains("roots"))
                ?: device.findObject(By.desc("Navigate up"))
        assertNotNull("Android DocumentsUI did not expose its roots navigation control", rootButton)
        rootButton.click()
        val root = device.wait(
            Until.findObject(By.text(OstudyTestDocumentsProvider.ROOT_TITLE)),
            10_000
        )
        assertNotNull("Android DocumentsUI did not expose the ostudy test provider root", root)
        root.click()
    }

    private fun pressSave(device: UiDevice, pickerPackages: Set<String>) {
        for (packageName in pickerPackages) {
            device.findObject(By.res(packageName, "save"))?.let {
                it.click()
                return
            }
        }
        val byText = device.findObject(By.text("Save")) ?: device.findObject(By.text("Create"))
        assertNotNull("Android DocumentsUI did not expose its save action", byText)
        byText.click()
    }

    private fun waitForSettledOperation(
        webView: WebView,
        sequence: Int,
        label: String
    ): JSONObject {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var latest = JSONObject()
        while (System.nanoTime() < deadline) {
            latest = evaluateJsonObject(
                webView,
                "window.__OSTUDY_ANDROID_TEST__.fileOperationSnapshot()"
            )
            if (latest.optInt("sequence") == sequence) {
                when (latest.optString("phase")) {
                    "settled" -> return latest
                    "error" -> throw AssertionError(
                        "$label failed: ${latest.optString("error", "unknown error")}"
                    )
                }
            }
            Thread.sleep(100)
        }
        throw AssertionError("$label did not settle. Last snapshot: $latest")
    }

    private fun waitForExportedFile(context: android.content.Context): java.io.File {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (System.nanoTime() < deadline) {
            OstudyTestDocumentsProvider.exportedFiles(context).firstOrNull()?.let { return it }
            Thread.sleep(100)
        }
        throw AssertionError("The Android test DocumentsProvider did not receive exported bytes.")
    }

    private fun waitForPicker(
        device: UiDevice,
        pickerPackages: Set<String>,
        label: String
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20)
        while (System.nanoTime() < deadline) {
            if (pickerVisible(device, pickerPackages)) return
            Thread.sleep(100)
        }
        throw AssertionError("$label did not open. candidatePackages=$pickerPackages")
    }

    private fun waitForPickerClosed(
        device: UiDevice,
        pickerPackages: Set<String>,
        label: String
    ) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (System.nanoTime() < deadline) {
            if (!pickerVisible(device, pickerPackages)) return
            Thread.sleep(100)
        }
        throw AssertionError("$label did not close")
    }

    private fun documentPickerPackages(action: String): Set<String> {
        val packageManager = InstrumentationRegistry.getInstrumentation().targetContext.packageManager
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

    private fun withApplication(block: (WebView) -> Unit) {
        val scenario = ActivityScenario.launch(MainActivity::class.java)
        block(waitForWebView(scenario))
    }

    private fun waitForWebView(scenario: ActivityScenario<MainActivity>): WebView {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15)
        var webView: WebView? = null
        var latestViewTree = "Activity has not been inspected yet."
        while (System.nanoTime() < deadline) {
            scenario.onActivity { activity ->
                val decorView = activity.window.decorView
                webView = findWebView(decorView)
                latestViewTree = describeViewTree(decorView)
            }
            webView?.let { return it }
            Thread.sleep(50)
        }
        throw AssertionError(
            "Timed out waiting for the Tauri WebView to attach. Last view tree: $latestViewTree"
        )
    }

    private fun waitForBridge(webView: WebView) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var latest = JSONObject()
        while (System.nanoTime() < deadline) {
            val encoded = evaluateSync(
                webView,
                """
                (() => {
                  const root = document.documentElement;
                  return JSON.stringify({
                    type: typeof window.__OSTUDY_ANDROID_TEST__,
                    state: root?.dataset?.androidTestBridge || (root ? 'unset' : 'no-document-root'),
                    error: root?.dataset?.androidTestBridgeError || '',
                    readyState: document.readyState
                  });
                })()
                """.trimIndent()
            )
            val decoded = decodeJavascriptString(encoded)
            if (decoded == "null") {
                Thread.sleep(100)
                continue
            }
            latest = JSONObject(decoded)
            if (latest.optString("type") == "object") return
            if (latest.optString("state") in setOf("missing", "error")) {
                throw AssertionError("Android smoke bridge initialization failed: $latest")
            }
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for Android smoke bridge. Last snapshot: $latest")
    }

    private fun evaluateJsonObject(webView: WebView, expression: String): JSONObject {
        val encoded = evaluateSync(webView, "JSON.stringify($expression)")
        return JSONObject(decodeJavascriptString(encoded))
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

    private fun findWebView(activity: Activity): WebView? = findWebView(activity.window.decorView)

    private fun findWebView(view: View): WebView? {
        if (view is WebView) return view
        if (view is ViewGroup) {
            for (index in 0 until view.childCount) {
                findWebView(view.getChildAt(index))?.let { return it }
            }
        }
        return null
    }

    private fun describeViewTree(root: View): String {
        val nodes = mutableListOf<String>()
        collectViewTree(root, 0, nodes)
        return nodes.joinToString(" | ")
    }

    private fun collectViewTree(view: View, depth: Int, nodes: MutableList<String>) {
        if (nodes.size >= 40) return
        nodes.add("${"  ".repeat(depth)}${view.javaClass.simpleName}")
        if (view is ViewGroup) {
            for (index in 0 until view.childCount) {
                collectViewTree(view.getChildAt(index), depth + 1, nodes)
                if (nodes.size >= 40) return
            }
        }
    }
}
