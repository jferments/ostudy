package io.github.jferments.ostudy

import android.os.Bundle
import android.view.View
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  /**
   * ostudy owns Android Back through the private platform plugin and the shared
   * React Back-handler stack. Wry otherwise registers its own browser-history
   * OnBackPressedCallback, which can consume the physical gesture before the
   * application navigation handler receives it.
   */
  override val handleBackNavigation: Boolean = false

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
    installContentInsets()
  }

  /**
   * The generated Tauri WebView is measured inside Android-owned system and IME
   * insets. This keeps every shared fixed/sticky surface inside the status bar,
   * navigation bar, display cutout, and software keyboard without teaching each
   * React screen about Android view hierarchy details.
   *
   * Insets are consumed before they reach the WebView because the shared shell
   * already has a CSS safe-area path for platforms that expose CSS env() values.
   * Leaving the raw Android values in both layers would apply them twice. IME
   * visibility is preserved so the JavaScript shell can hide compact navigation
   * while native layout owns the actual keyboard avoidance distance.
   */
  private fun installContentInsets() {
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
      val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
      val system = Insets.of(
        maxOf(bars.left, cutout.left),
        maxOf(bars.top, cutout.top),
        maxOf(bars.right, cutout.right),
        maxOf(bars.bottom, cutout.bottom),
      )
      val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      val bottom = if (imeVisible) maxOf(system.bottom, ime.bottom) else system.bottom

      if (
        view.paddingLeft != system.left ||
        view.paddingTop != system.top ||
        view.paddingRight != system.right ||
        view.paddingBottom != bottom
      ) {
        view.setPadding(system.left, system.top, system.right, bottom)
      }

      WindowInsetsCompat.Builder(insets)
        .setInsets(WindowInsetsCompat.Type.systemBars(), Insets.NONE)
        .setInsets(WindowInsetsCompat.Type.displayCutout(), Insets.NONE)
        .setInsets(WindowInsetsCompat.Type.ime(), Insets.NONE)
        .setVisible(WindowInsetsCompat.Type.ime(), imeVisible)
        .build()
    }
    ViewCompat.requestApplyInsets(content)
  }
}
