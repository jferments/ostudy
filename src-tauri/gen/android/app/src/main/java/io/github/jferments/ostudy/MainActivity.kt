package io.github.jferments.ostudy

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

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
  }
}
