package io.github.jferments.ostudy

import android.view.View
import android.view.WindowManager
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidInsetsTest {
  @Test
  fun contentRootOwnsSystemBarAndDisplayCutoutInsets() {
    // The emulator harness force-stops the application after this isolated
    // instrumentation case. Do not ActivityScenario.close() a live Tauri/Wry
    // Activity from AndroidJUnitRunner; that races the native event-loop teardown.
    val scenario = ActivityScenario.launch(MainActivity::class.java)
    val snapshot = waitForInsets(scenario)
    assertTrue("The API 36 emulator should expose a non-zero status-bar inset.", snapshot.expectedTop > 0)
    assertEquals(snapshot.expectedLeft, snapshot.paddingLeft)
    assertEquals(snapshot.expectedTop, snapshot.paddingTop)
    assertEquals(snapshot.expectedRight, snapshot.paddingRight)
    assertEquals(snapshot.expectedBottom, snapshot.paddingBottom)
  }

  @Test
  fun activityRequestsResizeSemanticsForTheSoftwareKeyboard() {
    val scenario = ActivityScenario.launch(MainActivity::class.java)
    var softInputMode = 0
    scenario.onActivity { activity -> softInputMode = activity.window.attributes.softInputMode }
    assertEquals(
      WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE,
      softInputMode and WindowManager.LayoutParams.SOFT_INPUT_MASK_ADJUST,
    )
  }

  private fun waitForInsets(scenario: ActivityScenario<MainActivity>): InsetsSnapshot {
    val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
    var latest = InsetsSnapshot()
    while (System.nanoTime() < deadline) {
      scenario.onActivity { activity ->
        val content = activity.findViewById<View>(android.R.id.content)
        val insets = ViewCompat.getRootWindowInsets(content)
        if (insets != null) {
          val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
          val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
          latest = InsetsSnapshot(
            expectedLeft = maxOf(bars.left, cutout.left),
            expectedTop = maxOf(bars.top, cutout.top),
            expectedRight = maxOf(bars.right, cutout.right),
            expectedBottom = maxOf(bars.bottom, cutout.bottom),
            paddingLeft = content.paddingLeft,
            paddingTop = content.paddingTop,
            paddingRight = content.paddingRight,
            paddingBottom = content.paddingBottom,
          )
        }
      }
      if (
        latest.expectedTop > 0 &&
        latest.expectedLeft == latest.paddingLeft &&
        latest.expectedTop == latest.paddingTop &&
        latest.expectedRight == latest.paddingRight &&
        latest.expectedBottom == latest.paddingBottom
      ) {
        return latest
      }
      Thread.sleep(50)
    }
    throw AssertionError("Android content insets did not settle before the timeout: $latest")
  }

  private data class InsetsSnapshot(
    val expectedLeft: Int = -1,
    val expectedTop: Int = -1,
    val expectedRight: Int = -1,
    val expectedBottom: Int = -1,
    val paddingLeft: Int = -2,
    val paddingTop: Int = -2,
    val paddingRight: Int = -2,
    val paddingBottom: Int = -2,
  )
}
