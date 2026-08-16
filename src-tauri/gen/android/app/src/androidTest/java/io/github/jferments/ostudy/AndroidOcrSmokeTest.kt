package io.github.jferments.ostudy

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidOcrSmokeTest {
  @Test
  fun bundledMlKitRecognizesHighContrastEnglishTextOffline() {
    val bitmap = Bitmap.createBitmap(1_600, 420, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    canvas.drawColor(Color.WHITE)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
      color = Color.BLACK
      textSize = 150f
      typeface = Typeface.create(Typeface.SANS_SERIF, Typeface.BOLD)
    }
    canvas.drawText("OSTUDY FLASHCARDS", 70f, 260f, paint)

    val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
    try {
      val result = Tasks.await(
        recognizer.process(InputImage.fromBitmap(bitmap, 0)),
        20,
        TimeUnit.SECONDS,
      )
      val recognized = result.text.uppercase()
      assertTrue(
        "Expected ML Kit to recognize the synthetic OSTUDY line, got: $recognized",
        recognized.contains("OSTUDY"),
      )
      assertTrue(
        "Expected ML Kit to recognize the synthetic FLASHCARDS line, got: $recognized",
        recognized.contains("FLASH"),
      )
    } finally {
      recognizer.close()
      bitmap.recycle()
    }
  }
}
