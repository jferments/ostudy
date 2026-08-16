package io.github.jferments.ostudy

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import com.googlecode.tesseract.android.TessBaseAPI
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidTesseractRecognitionTest {
  @Test
  fun bundledTesseractRecognizesAHighContrastEnglishLineOffline() {
    val context = ApplicationProvider.getApplicationContext<android.content.Context>()
    val dataRoot = File(context.noBackupFilesDir, "tesseract-instrumentation")
    val tessdata = File(dataRoot, "tessdata")
    tessdata.mkdirs()
    val model = File(tessdata, "eng.traineddata")
    if (!model.isFile || model.length() < 1_000_000L) {
      context.assets.open("tessdata/eng.traineddata").use { input ->
        model.outputStream().use(input::copyTo)
      }
    }

    val bitmap = Bitmap.createBitmap(1_600, 420, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    canvas.drawColor(Color.WHITE)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
      color = Color.BLACK
      textSize = 150f
      typeface = Typeface.create(Typeface.SANS_SERIF, Typeface.BOLD)
    }
    canvas.drawText("OSTUDY FLASHCARDS", 70f, 260f, paint)

    val api = TessBaseAPI()
    try {
      assertTrue("Tesseract should initialize from the bundled English model.", api.init(dataRoot.path, "eng"))
      api.setImage(bitmap)
      val recognized = api.utF8Text.orEmpty().uppercase()
      assertTrue(
        "Expected Tesseract to recognize the synthetic OSTUDY line, got: $recognized",
        recognized.contains("OSTUDY") && recognized.contains("FLASH"),
      )
      assertTrue("The recognized line should have a positive mean confidence.", api.meanConfidence() > 0)
    } finally {
      api.end()
      bitmap.recycle()
      dataRoot.deleteRecursively()
    }
  }
}
