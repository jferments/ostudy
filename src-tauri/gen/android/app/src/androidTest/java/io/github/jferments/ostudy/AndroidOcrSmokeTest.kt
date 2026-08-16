package io.github.jferments.ostudy

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.io.FileOutputStream

@RunWith(AndroidJUnit4::class)
class AndroidOcrSmokeTest {
  @Test
  fun bundledTesseractModelInitializesAndRecognizesRenderedEnglishText() {
    val context = InstrumentationRegistry.getInstrumentation().targetContext
    val dataRoot = File(context.noBackupFilesDir, "instrumentation-tesseract")
    val model = File(dataRoot, "tessdata/eng.traineddata")
    model.parentFile?.mkdirs()
    context.assets.open("tessdata/eng.traineddata").use { input ->
      FileOutputStream(model, false).use(input::copyTo)
    }
    assertTrue("The bundled English model must be substantive.", model.length() > 1_000_000L)

    val bitmap = Bitmap.createBitmap(1_400, 320, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(bitmap)
    canvas.drawColor(Color.WHITE)
    canvas.drawText(
      "NURSING STUDY 123",
      60f,
      210f,
      Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.BLACK
        textSize = 128f
      },
    )

    val apiClass = Class.forName("cz.adaptech.tesseract4android.TessBaseAPI")
    val api = apiClass.getConstructor().newInstance()
    try {
      val initialized = apiClass
        .getMethod("init", String::class.java, String::class.java)
        .invoke(api, dataRoot.absolutePath, "eng") as Boolean
      assertTrue("Tesseract must initialize from the bundled model.", initialized)
      apiClass.getMethod("setPageSegMode", Integer.TYPE).invoke(api, 6)
      apiClass.getMethod("setImage", Bitmap::class.java).invoke(api, bitmap)
      val text = (apiClass.getMethod("getUTF8Text").invoke(api) as String).uppercase()
      assertTrue("Tesseract should recognize the rendered nursing label: $text", text.contains("NURSING"))
      assertTrue("Tesseract should recognize the rendered study label: $text", text.contains("STUDY"))
    } finally {
      runCatching { apiClass.getMethod("recycle").invoke(api) }
      bitmap.recycle()
      dataRoot.deleteRecursively()
    }
  }
}
