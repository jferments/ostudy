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
    fun a1_freshLibraryUsesQuietNavigation() {
        withApplication { webView ->
            waitForBridge(webView)
            // A fresh library opens on Home, which welcomes it instead of showing
            // an empty Today strip or empty subject tiles.
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Home' && " +
                        "Array.from(document.querySelectorAll('main h2')).some(heading => heading.textContent?.trim() === 'Your library is empty')"
                }
            )
            val homeSnapshot = evaluateJsonObject(
                webView,
                """
                (() => {
                  const navigation = document.querySelector("nav[aria-label='Main navigation']");
                  const items = [...(navigation?.querySelectorAll('button') ?? [])];
                  const buttons = [...document.querySelectorAll('main button')];
                  const has = (label) => buttons.some((button) => button.textContent?.trim() === label);
                  return {
                    navigation: items
                      .map((button) => (button.querySelector("[data-slot='nav-label']")?.textContent || '').trim())
                      .join('|'),
                    current: items
                      .filter((button) => button.getAttribute('aria-current') === 'page')
                      .map((button) => button.dataset.testid || '')
                      .join('|'),
                    bottomNavigation: navigation?.dataset.slot === 'bottom-navigation',
                    searchAction: Boolean(document.querySelector('header [data-testid="app-search"]')),
                    settingsGear: Boolean(document.querySelector('header [data-testid="app-settings"]')),
                    createSubject: has('Create a subject'),
                    importDocument: has('Import a document'),
                    openLibrary: has('Open Library'),
                    todayStrip: Boolean(document.querySelector('main [data-testid="home-today"]')),
                    subjectTiles: document.querySelectorAll('main [data-testid="subject-tile"]').length
                  };
                })()
                """.trimIndent()
            )
            assertEquals("Home|Library|Review|Tutor|Progress", homeSnapshot.getString("navigation"))
            assertEquals("nav-home", homeSnapshot.getString("current"))
            assertTrue(homeSnapshot.getBoolean("bottomNavigation"))
            assertTrue(homeSnapshot.getBoolean("searchAction"))
            assertTrue(homeSnapshot.getBoolean("settingsGear"))
            assertTrue(homeSnapshot.getBoolean("createSubject"))
            assertTrue(homeSnapshot.getBoolean("importDocument"))
            assertTrue(homeSnapshot.getBoolean("openLibrary"))
            assertFalse(homeSnapshot.getBoolean("todayStrip"))
            assertEquals(0, homeSnapshot.getInt("subjectTiles"))

            // Open Library leads to the Workbench, which opens on its tree on a
            // phone. An empty library offers one way to start a deck there and
            // no controls that only mean something with decks in it.
            evaluateSync(
                webView,
                "Array.from(document.querySelectorAll('main button')).find(button => button.textContent?.trim() === 'Open Library')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('main [data-testid=\"library-workbench\"] nav[aria-label=\"Library\"]')) && " +
                        "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'New deck')"
                }
            )
            val snapshot = evaluateJsonObject(
                webView,
                """
                (() => {
                  const tree = document.querySelector("main [data-testid='library-workbench'] nav[aria-label='Library']");
                  const sections = [...(tree?.querySelectorAll('section[aria-label]') ?? [])];
                  const pages = document.querySelector("main nav[aria-label='Library pages']");
                  const buttons = [...document.querySelectorAll('main button')];
                  const label = (button) => (button.getAttribute('aria-label') || button.textContent || '').trim();
                  return {
                    sections: sections.map((section) => section.getAttribute('aria-label')).join('|'),
                    libraryCurrent: document.querySelector('[data-testid="nav-library"]')?.getAttribute('aria-current') === 'page',
                    settingsGear: Boolean(document.querySelector('[data-testid="app-settings"]')),
                    createActions: buttons.filter((button) => button.textContent?.trim() === 'New deck').length,
                    importAction: buttons.some((button) => label(button) === 'Import deck'),
                    documentsView: Boolean(pages?.querySelector('[data-testid="library-view-documents"]')),
                    studyGuidesView: Boolean(pages?.querySelector('[data-testid="library-view-study-guides"]')),
                    cardList: Boolean(document.querySelector("main section[aria-label='Cards']")),
                    irrelevantControls: buttons.some((button) => {
                      const text = label(button);
                      return text === 'Collapse all' || text === 'Expand all' ||
                        text.startsWith('Select all') || text.startsWith('Expand ') || text.startsWith('Collapse ');
                    })
                  };
                })()
                """.trimIndent()
            )
            assertEquals("Subjects", snapshot.getString("sections"))
            assertTrue(snapshot.getBoolean("libraryCurrent"))
            assertTrue(snapshot.getBoolean("settingsGear"))
            assertEquals(1, snapshot.getInt("createActions"))
            assertTrue(snapshot.getBoolean("importAction"))
            assertTrue(snapshot.getBoolean("documentsView"))
            assertTrue(snapshot.getBoolean("studyGuidesView"))
            assertFalse(snapshot.getBoolean("cardList"))
            assertFalse(snapshot.getBoolean("irrelevantControls"))
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
    fun c_documentsViewAndAndroidBackReturnHome() {
        withApplication { webView ->
            waitForBridge(webView)
            evaluateSync(
                webView,
                "document.querySelector('[data-testid=\"nav-library\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(document.querySelector('main nav[aria-label=\"Library pages\"] [data-testid=\"library-view-documents\"]'))"
                }
            )
            evaluateSync(
                webView,
                "document.querySelector('main nav[aria-label=\"Library pages\"] [data-testid=\"library-view-documents\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Documents' && " +
                        "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'Upload document') && " +
                        "document.querySelector('[data-testid=\"library-view-documents\"]')?.getAttribute('aria-current') === 'page'"
                }
            )
            waitForAndroidBackReady(webView)
            UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Home' && " +
                        "document.querySelector('[data-testid=\"nav-home\"]')?.getAttribute('aria-current') === 'page'"
                }
            )
            // Home is the root: the app keeps no Back handler there, so Back leaves to Android.
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.documentElement.dataset.androidBackInterception === 'false'"
                }
            )
        }
    }

    @Test
    fun c_tutorOfflineAndAndroidBackReturnHome() {
        withApplication { webView ->
            waitForBridge(webView)
            evaluateSync(
                webView,
                "document.querySelector('[data-testid=\"nav-tutor\"]')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Tutor' && " +
                        "Boolean(document.querySelector('main [data-testid=\"tutor-screen\"] textarea')) && " +
                        "!document.querySelector('main [role=\"alert\"]')"
                }
            )
            val conversations = evaluatePromise(
                webView,
                "window.__TAURI_INTERNALS__.invoke('list_tutor_conversations').then(items => ({count: items.length}))"
            )
            assertEquals(0, conversations.getInt("count"))
            waitForAndroidBackReady(webView)
            UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Home' && " +
                        "document.querySelector('[data-testid=\"nav-home\"]')?.getAttribute('aria-current') === 'page'"
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
                "document.querySelector('[data-testid=\"nav-library\"]')?.click(); true"
            )
            // Library reopens the page used last. Step to the Workbench's tree from
            // another Library page or from a deck's card list, then open the deck.
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "(() => { " +
                        "const deck = Array.from(document.querySelectorAll('main nav[aria-label=\"Library\"] button')).find(button => button.querySelector('span')?.textContent?.trim() === 'Android Emulator Smoke Deck'); " +
                        "if (deck) return true; " +
                        "const page = document.querySelector('main nav[aria-label=\"Library pages\"] [data-testid=\"library-view-decks\"]'); " +
                        "if (page) { page.click(); return false; } " +
                        "document.querySelector('main [data-testid=\"library-workbench\"] button[aria-label=\"Library\"]')?.click(); " +
                        "return false; " +
                        "})()"
                }
            )
            evaluateSync(webView, "Array.from(document.querySelectorAll('main nav[aria-label=\"Library\"] button')).find(button => button.querySelector('span')?.textContent?.trim() === 'Android Emulator Smoke Deck')?.click(); true")
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1[data-testid=\"workbench-title\"]')?.textContent?.trim() === 'Android Emulator Smoke Deck'"
                }
            )
            evaluateSync(
                webView,
                "Array.from(document.querySelectorAll('main button')).find(button => button.textContent?.trim() === 'New card')?.click(); true"
            )
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Array.from(document.querySelectorAll('main h1')).some(heading => heading.textContent?.trim() === 'Add card') && " +
                        "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'Save card')"
                }
            )
            waitForAndroidBackReady(webView)
            device.pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1[data-testid=\"workbench-title\"]')?.textContent?.trim() === 'Android Emulator Smoke Deck' && " +
                        "!Array.from(document.querySelectorAll('main h1')).some(heading => heading.textContent?.trim() === 'Add card')"
                }
            )
            waitForAndroidBackReady(webView)
            device.pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "Boolean(Array.from(document.querySelectorAll('main nav[aria-label=\"Library\"] button')).find(button => button.querySelector('span')?.textContent?.trim() === 'Android Emulator Smoke Deck')) && " +
                        "!document.querySelector('main h1[data-testid=\"workbench-title\"]')"
                }
            )
            // With the nested work closed, Back leaves Library for Home.
            waitForAndroidBackReady(webView)
            device.pressBack()
            waitUntil(
                webView,
                predicate = { it == "true" },
                expression = {
                    "document.querySelector('main h1')?.textContent?.trim() === 'Home' && " +
                        "document.querySelector('[data-testid=\"nav-home\"]')?.getAttribute('aria-current') === 'page'"
                }
            )
        }
    }

    @Test
    fun d_reloadsIntoInterruptedPracticeSession() {
        withApplication { webView ->
            waitForBridge(webView)
            val started = evaluatePromise(webView, "window.__OSTUDY_ANDROID_TEST__.beginInterruptedPractice()")
            val sessionId = started.getString("sessionId")
            evaluateSync(webView, "delete window.__OSTUDY_ANDROID_TEST__; window.location.reload(); true")
            waitForBridge(webView)
            waitUntil(webView, predicate = { it == "true" }, expression = {
                "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'End session') && " +
                    "Array.from(document.querySelectorAll('nav[aria-label=\"Main navigation\"] button[aria-current=\"page\"]')).map(button => button.dataset.testid).join('|') === 'nav-review'"
            })
            val restored = evaluatePromise(webView, "window.__OSTUDY_ANDROID_TEST__.restorationSnapshot()")
            assertEquals(sessionId, restored.getString("sessionId"))
            assertTrue(restored.getBoolean("revealed"))
            assertFalse(restored.getBoolean("completed"))
        }
    }

    @Test
    fun e_processRestartKeepsInterruptedPracticeSession() {
        withApplication { webView ->
            waitForBridge(webView)
            waitUntil(webView, predicate = { it == "true" }, expression = {
                "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'End session') && " +
                    "Array.from(document.querySelectorAll('nav[aria-label=\"Main navigation\"] button[aria-current=\"page\"]')).map(button => button.dataset.testid).join('|') === 'nav-review'"
            })
            val restored = evaluatePromise(webView, "window.__OSTUDY_ANDROID_TEST__.restorationSnapshot()")
            assertNotNull(restored.optString("sessionId").takeIf(String::isNotBlank))
            assertTrue(restored.getBoolean("revealed"))
            assertFalse(restored.getBoolean("completed"))
            evaluateSync(webView, "Array.from(document.querySelectorAll('main button')).find(button => button.textContent?.trim() === 'End session')?.click(); true")
            waitUntil(webView, predicate = { it == "true" }, expression = {
                "document.querySelector('main h1')?.textContent?.trim() === 'Study Guides' && " +
                    "document.querySelector('[data-testid=\"nav-library\"]')?.getAttribute('aria-current') === 'page' && " +
                    "document.querySelector('[data-testid=\"library-view-study-guides\"]')?.getAttribute('aria-current') === 'page'"
            })
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
            cancelFileOperation(webView, device, "window.__OSTUDY_ANDROID_TEST__.startDeckExportPicker()",
                action = Intent.ACTION_CREATE_DOCUMENT, timeoutMillis = 30_000, label = "Android deck export picker")
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
                cancelFileOperation(webView, device, "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')",
                    action = Intent.ACTION_OPEN_DOCUMENT, timeoutMillis = 20_000, label = label)
            }
            cancelFileOperation(webView, device, "window.__OSTUDY_ANDROID_TEST__.startDeckExportPicker()",
                action = Intent.ACTION_CREATE_DOCUMENT, timeoutMillis = 30_000, label = "Android deck export picker")
        }
    }

    private fun assertImportPickerCancels(purpose: String, label: String) {
        withApplication { webView ->
            waitForBridge(webView)
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            cancelFileOperation(webView, device, "window.__OSTUDY_ANDROID_TEST__.startImportPicker('$purpose')",
                action = Intent.ACTION_OPEN_DOCUMENT, timeoutMillis = 20_000, label = label)
        }
    }
    private fun cancelFileOperation(webView: WebView, device: UiDevice, expression: String,
        action: String, timeoutMillis: Long, label: String) {
        val pickerPackages = documentPickerPackages(action)
        val sequence = startFileOperation(webView, expression)
        waitForPickerOrFailure(webView, device, sequence, pickerPackages, timeoutMillis, label)
        device.pressBack()
        waitForPickerClosed(device, pickerPackages, label)
        waitForBridge(webView)
        waitForFileOperationCancelled(webView, sequence, label)
    }
    private fun startFileOperation(webView: WebView, expression: String): Int {
        val snapshot = evaluateJsonObject(webView, expression)
        assertEquals(snapshot.optString("error"), "pending", snapshot.getString("phase"))
        return snapshot.getInt("sequence")
    }
    private fun waitForPickerOrFailure(webView: WebView, device: UiDevice, sequence: Int,
        pickerPackages: Set<String>, timeoutMillis: Long, label: String) {
        val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMillis)
        var latest = fileOperationSnapshot(webView)
        while (System.nanoTime() < deadline) {
            if (pickerVisible(device, pickerPackages)) return
            latest = fileOperationSnapshot(webView)
            if (latest.optInt("sequence") == sequence && latest.optString("phase") == "error") {
                throw AssertionError("$label failed before opening: ${latest.optString("error", "unknown error")}")
            }
            if (latest.optInt("sequence") == sequence && latest.optString("phase") == "settled") {
                throw AssertionError("$label settled before its system activity was observed: $latest")
            }
            Thread.sleep(100)
        }
        throw AssertionError("$label did not open. candidatePackages=$pickerPackages " +
            "operationPhase=${latest.optString("phase")} operationError=${latest.optString("error", "none")}")
    }
    private fun waitForPickerClosed(device: UiDevice, pickerPackages: Set<String>, label: String) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (System.nanoTime() < deadline) {
            if (!pickerVisible(device, pickerPackages)) return
            Thread.sleep(100)
        }
        throw AssertionError("$label did not close after Android Back")
    }
    private fun waitForFileOperationCancelled(webView: WebView, sequence: Int, label: String) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20)
        var latest = fileOperationSnapshot(webView)
        while (System.nanoTime() < deadline) {
            latest = fileOperationSnapshot(webView)
            if (latest.optInt("sequence") == sequence) {
                when (latest.optString("phase")) {
                    "settled" -> {
                        assertTrue("$label resolved without reporting cancellation: $latest", latest.getBoolean("cancelled"))
                        return
                    }
                    "error" -> throw AssertionError("$label failed after picker result: " + latest.optString("error", "unknown error"))
                }
            }
            Thread.sleep(100)
        }
        throw AssertionError("$label did not settle after cancellation. " +
            "operationPhase=${latest.optString("phase")} operationError=${latest.optString("error", "none")}")
    }
    private fun documentPickerPackages(action: String): Set<String> {
        val packageManager = InstrumentationRegistry.getInstrumentation().targetContext.packageManager
        val intent = Intent(action).apply { addCategory(Intent.CATEGORY_OPENABLE); type = "*/*" }
        val resolved = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            packageManager.queryIntentActivities(intent, PackageManager.ResolveInfoFlags.of(PackageManager.MATCH_DEFAULT_ONLY.toLong()))
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
        evaluateJsonObject(webView, "window.__OSTUDY_ANDROID_TEST__.fileOperationSnapshot()")
    private fun evaluateJsonObject(webView: WebView, expression: String): JSONObject {
        val encoded = evaluateSync(webView, "JSON.stringify($expression)")
        return JSONObject(decodeJavascriptString(encoded))
    }
    private fun waitForAndroidBackReady(webView: WebView) {
        waitUntil(webView, predicate = { it == "true" }, expression = {
            "document.documentElement?.dataset != null && document.documentElement.dataset.androidBackInterception === 'true' && document.documentElement.dataset.overlayActive !== 'true'"
        })
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
        throw AssertionError("Timed out waiting for the Tauri WebView to attach. Last view tree: $latestViewTree")
    }
    private fun describeViewTree(root: View): String {
        val nodes = mutableListOf<String>()
        collectViewTree(root, depth = 0, nodes = nodes)
        return nodes.joinToString(separator = " | ")
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
    private fun waitForBridge(webView: WebView) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(30)
        var latest = JSONObject().put("type", "unavailable").put("state", "not-evaluated").put("error", "").put("readyState", "unknown")
        while (System.nanoTime() < deadline) {
            val encoded = evaluateSync(webView, """
                (() => {
                  const root = document.documentElement;
                  return JSON.stringify({
                    type: typeof window.__OSTUDY_ANDROID_TEST__,
                    state: root?.dataset?.androidTestBridge || (root ? 'unset' : 'no-document-root'),
                    error: root?.dataset?.androidTestBridgeError || '',
                    readyState: document.readyState
                  });
                })()
                """.trimIndent())
            val decoded = decodeJavascriptString(encoded)
            if (decoded == "null") {
                latest = JSONObject().put("type", "unavailable").put("state", "no-evaluation-result").put("error", "").put("readyState", "unknown")
                Thread.sleep(100)
                continue
            }
            latest = JSONObject(decoded)
            if (latest.optString("type") == "object") return
            val state = latest.optString("state")
            if (state == "missing" || state == "error") throw AssertionError("Android smoke bridge initialization failed: $latest")
            Thread.sleep(100)
        }
        throw AssertionError("Timed out waiting for Android smoke bridge. Last snapshot: $latest")
    }
    private fun evaluatePromise(webView: WebView, expression: String): JSONObject {
        val key = "__ostudySmoke" + UUID.randomUUID().toString().replace("-", "")
        evaluateSync(webView, """
            (() => {
              window['$key'] = null;
              Promise.resolve($expression)
                .then(value => { window['$key'] = JSON.stringify({ ok: true, value }); })
                .catch(error => { window['$key'] = JSON.stringify({ ok: false, error: String(error) }); });
              return true;
            })()
            """.trimIndent())
        val envelope = waitUntil(webView, timeoutSeconds = 45, predicate = { it != "null" }, expression = { "window['$key']" })
        val parsed = JSONObject(decodeJavascriptString(envelope))
        assertEquals(parsed.optString("error"), true, parsed.getBoolean("ok"))
        return parsed.getJSONObject("value")
    }
    private fun waitUntil(webView: WebView, timeoutSeconds: Long = 15, predicate: (String) -> Boolean, expression: () -> String): String {
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
            webView.evaluateJavascript(script) { value -> result = value ?: "null"; latch.countDown() }
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
            for (index in 0 until view.childCount) findWebView(view.getChildAt(index))?.let { return it }
        }
        return null
    }
}
