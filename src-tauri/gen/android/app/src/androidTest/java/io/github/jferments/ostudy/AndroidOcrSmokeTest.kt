package io.github.jferments.ostudy

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Typeface
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import com.googlecode.tesseract.android.TessBaseAPI
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.io.FileOutputStream

@RunWith(AndroidJUnit4::class)
@LargeTest
class AndroidOcrSmokeTest {
  @Test
  fun bundledTesseractRecognizesHighContrastEnglishTextOffline() {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val dataRoot = File(context.filesDir, "tesseract-smoke")
    val trainedData = File(dataRoot, "tessdata/eng.traineddata")
    trainedData.parentFile?.mkdirs()
    context.assets.open("tessdata/eng.traineddata").use { input ->
      FileOutputStream(trainedData, false).use { output ->
        input.copyTo(output)
        output.fd.sync()
      }
    }
    assertTrue("Packaged Tesseract English data is empty", trainedData.length() > 0L)

    val bitmap = Bitmap.createBitmap(1_600, 420, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    canvas.drawColor(Color.WHITE)
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
      color = Color.BLACK
      textSize = 150f
      typeface = Typeface.create(Typeface.SANS_SERIF, Typeface.BOLD)
    }
    canvas.drawText("OSTUDY FLASHCARDS", 70f, 260f, paint)

    val tess = TessBaseAPI()
    try {
      assertTrue(
        "Expected bundled English Tesseract data to initialize",
        tess.init(dataRoot.absolutePath, "eng", TessBaseAPI.OEM_LSTM_ONLY),
      )
      tess.setPageSegMode(TessBaseAPI.PageSegMode.PSM_AUTO)
      tess.setImage(bitmap)
      tess.getHOCRText(0)
      val recognized = (tess.getUTF8Text() ?: "").uppercase()
      assertTrue(
        "Expected Tesseract to recognize the synthetic OSTUDY line, got: $recognized",
        recognized.contains("OSTUDY"),
      )
      assertTrue(
        "Expected Tesseract to recognize the synthetic FLASHCARDS line, got: $recognized",
        recognized.contains("FLASH"),
      )
    } finally {
      tess.recycle()
      bitmap.recycle()
      dataRoot.deleteRecursively()
    }
  }
}
