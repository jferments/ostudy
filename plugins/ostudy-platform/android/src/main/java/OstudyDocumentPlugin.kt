package io.github.jferments.ostudy.platform

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.pdf.PdfRenderer
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.webkit.WebView
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.tasks.Tasks
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.Text
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import kotlin.math.ceil

private const val PDF_RENDER_ENGINE = "android-pdf-renderer-v1"
private const val OCR_ENGINE = "ml-kit-text-recognition-v2-bundled-latin-16.0.1"
private const val MAX_PDF_PAGES = 500
private const val MIN_RENDER_DPI = 72
private const val MAX_RENDER_DPI = 300
private const val MAX_RENDER_DIMENSION = 12_000
private const val MAX_RENDER_PIXELS = 50_000_000L
private const val MAX_RENDER_BYTES = 64L * 1024L * 1024L
private const val MAX_OCR_SOURCE_BYTES = 64L * 1024L * 1024L
private const val MIN_OCR_TIMEOUT_SECONDS = 1
private const val MAX_OCR_TIMEOUT_SECONDS = 180
private const val PDF_POINTS_PER_INCH = 72.0

@InvokeArg
class OpenPdfArgs {
    lateinit var sourcePath: String
}

@InvokeArg
class RenderPdfPageArgs {
    lateinit var token: String
    var pageNumber: Int = 0
    var dpi: Int = 0
    lateinit var targetPath: String
    var maximumBytes: Long = 0
}

@InvokeArg
class ClosePdfArgs {
    lateinit var token: String
}

@InvokeArg
class RecognizeImageArgs {
    lateinit var sourcePath: String
    var language: String = "en"
    var timeoutSeconds: Int = 0
}

@TauriPlugin
class OstudyDocumentPlugin(private val activity: Activity) : Plugin(activity) {
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val sessions = ConcurrentHashMap<String, PdfSession>()

    override fun load(webView: WebView) {
        val owner = activity as? LifecycleOwner ?: return
        owner.lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                closeAllSessions()
                ioScope.cancel()
                owner.lifecycle.removeObserver(this)
            }
        })
    }

    @Command
    fun documentCapabilities(invoke: Invoke) {
        val result = JSObject()
        result.put("pdfRenderAvailable", true)
        result.put("ocrAvailable", true)
        result.put("rendererEngine", PDF_RENDER_ENGINE)
        result.put("ocrEngine", OCR_ENGINE)
        invoke.resolve(result)
    }

    @Command
    fun openPdf(invoke: Invoke) {
        val args = invoke.parseArgs(OpenPdfArgs::class.java)
        ioScope.launch {
            try {
                val source = validatePrivateFile(args.sourcePath, "PDF source")
                val descriptor = ParcelFileDescriptor.open(source, ParcelFileDescriptor.MODE_READ_ONLY)
                val renderer = try {
                    PdfRenderer(descriptor)
                } catch (error: Exception) {
                    descriptor.close()
                    throw error
                }
                val pageCount = renderer.pageCount
                if (pageCount !in 1..MAX_PDF_PAGES) {
                    renderer.close()
                    descriptor.close()
                    throw IllegalArgumentException(
                        "The PDF must contain between 1 and $MAX_PDF_PAGES pages."
                    )
                }
                val token = UUID.randomUUID().toString().replace("-", "")
                sessions[token] = PdfSession(descriptor, renderer)
                val response = JSObject()
                response.put("token", token)
                response.put("pageCount", pageCount)
                withContext(Dispatchers.Main) { invoke.resolve(response) }
            } catch (error: Exception) {
                withContext(Dispatchers.Main) {
                    invoke.reject("Android could not open this PDF: ${safeMessage(error)}")
                }
            }
        }
    }

    @Command
    fun renderPdfPage(invoke: Invoke) {
        val args = invoke.parseArgs(RenderPdfPageArgs::class.java)
        ioScope.launch {
            var target: File? = null
            try {
                validateToken(args.token)
                require(args.dpi in MIN_RENDER_DPI..MAX_RENDER_DPI) {
                    "The requested PDF render resolution is outside the supported range."
                }
                require(args.maximumBytes in 1..MAX_RENDER_BYTES) {
                    "The requested PDF render size limit is invalid."
                }
                val session = sessions[args.token]
                    ?: throw IllegalArgumentException("The PDF rendering session has expired.")
                target = validatePrivatePath(args.targetPath)
                target.parentFile?.mkdirs()
                val response = synchronized(session.lock) {
                    val pageCount = session.renderer.pageCount
                    require(args.pageNumber in 1..pageCount) {
                        "The requested page is outside the PDF page range."
                    }
                    session.renderer.openPage(args.pageNumber - 1).use { page ->
                        val width = scaledDimension(page.width, args.dpi)
                        val height = scaledDimension(page.height, args.dpi)
                        val pixels = width.toLong() * height.toLong()
                        require(pixels in 1..MAX_RENDER_PIXELS) {
                            "The requested page exceeds Android's bounded PDF bitmap budget."
                        }
                        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
                        try {
                            bitmap.eraseColor(Color.WHITE)
                            page.render(
                                bitmap,
                                null,
                                null,
                                PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY
                            )
                            FileOutputStream(target, false).use { output ->
                                require(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                                    "Android could not encode the rendered PDF page."
                                }
                                output.fd.sync()
                            }
                        } finally {
                            bitmap.recycle()
                        }
                        val byteSize = target.length()
                        require(byteSize in 1..args.maximumBytes) {
                            "The rendered PDF page exceeds ostudy's image safety limit."
                        }
                        JSObject().apply {
                            put("pageNumber", args.pageNumber)
                            put("width", width)
                            put("height", height)
                            put("byteSize", byteSize)
                        }
                    }
                }
                withContext(Dispatchers.Main) { invoke.resolve(response) }
            } catch (error: Exception) {
                target?.delete()
                withContext(Dispatchers.Main) {
                    invoke.reject(
                        "Android could not render PDF page ${args.pageNumber}: ${safeMessage(error)}"
                    )
                }
            }
        }
    }

    @Command
    fun closePdf(invoke: Invoke) {
        val args = invoke.parseArgs(ClosePdfArgs::class.java)
        ioScope.launch {
            try {
                validateToken(args.token)
                closeSession(args.token)
                withContext(Dispatchers.Main) { invoke.resolve() }
            } catch (error: Exception) {
                withContext(Dispatchers.Main) {
                    invoke.reject("Android could not close the PDF session: ${safeMessage(error)}")
                }
            }
        }
    }

    @Command
    fun recognizeImage(invoke: Invoke) {
        val args = invoke.parseArgs(RecognizeImageArgs::class.java)
        ioScope.launch {
            try {
                require(args.timeoutSeconds in MIN_OCR_TIMEOUT_SECONDS..MAX_OCR_TIMEOUT_SECONDS) {
                    "The OCR timeout is outside the supported range."
                }
                val source = validatePrivateFile(args.sourcePath, "OCR image")
                require(source.length() in 1..MAX_OCR_SOURCE_BYTES) {
                    "The OCR image exceeds ostudy's image safety limit."
                }
                val dimensions = imageDimensions(source)
                val pixels = dimensions.first.toLong() * dimensions.second.toLong()
                require(
                    dimensions.first in 1..MAX_RENDER_DIMENSION &&
                        dimensions.second in 1..MAX_RENDER_DIMENSION &&
                        pixels in 1..MAX_RENDER_PIXELS
                ) {
                    "The OCR image dimensions exceed Android's bounded image budget."
                }

                val started = System.nanoTime()
                val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
                val recognized = try {
                    val image = InputImage.fromFilePath(activity, Uri.fromFile(source))
                    Tasks.await(
                        recognizer.process(image),
                        args.timeoutSeconds.toLong(),
                        TimeUnit.SECONDS
                    )
                } finally {
                    recognizer.close()
                }
                val response = JSObject()
                response.put("text", recognized.text)
                response.put("confidence", meanConfidence(recognized))
                response.put("engine", OCR_ENGINE)
                response.put("language", dominantLanguage(recognized, args.language))
                response.put("width", dimensions.first)
                response.put("height", dimensions.second)
                response.put(
                    "durationMillis",
                    TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started)
                )
                withContext(Dispatchers.Main) { invoke.resolve(response) }
            } catch (error: Exception) {
                withContext(Dispatchers.Main) {
                    invoke.reject("Android on-device OCR failed: ${safeMessage(error)}")
                }
            }
        }
    }

    private fun scaledDimension(points: Int, dpi: Int): Int {
        val value = ceil(points.toDouble() * dpi.toDouble() / PDF_POINTS_PER_INCH).toInt()
        require(value in 1..MAX_RENDER_DIMENSION) {
            "The rendered PDF page dimension exceeds ostudy's image limit."
        }
        return value
    }

    private fun imageDimensions(source: File): Pair<Int, Int> {
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(source.path, options)
        require(options.outWidth > 0 && options.outHeight > 0) {
            "The selected OCR image is not a readable bitmap."
        }
        return options.outWidth to options.outHeight
    }

    private fun meanConfidence(text: Text): Double {
        var sum = 0.0
        var samples = 0
        for (block in text.textBlocks) {
            for (line in block.lines) {
                for (element in line.elements) {
                    val confidence = element.confidence.toDouble()
                    if (confidence.isFinite() && confidence > 0.0) {
                        sum += confidence.coerceIn(0.0, 1.0)
                        samples += 1
                    }
                }
            }
        }
        if (samples > 0) return (sum / samples.toDouble()).coerceIn(0.0, 1.0)
        return if (text.text.isBlank()) 0.0 else 0.5
    }

    private fun dominantLanguage(text: Text, requested: String): String {
        val counts = linkedMapOf<String, Int>()
        for (block in text.textBlocks) {
            for (line in block.lines) {
                for (element in line.elements) {
                    val language = element.recognizedLanguage
                    if (language.isNotBlank() && language != "und") {
                        counts[language] = (counts[language] ?: 0) + 1
                    }
                }
            }
        }
        return counts.maxByOrNull { it.value }?.key
            ?: requested.takeIf(String::isNotBlank)
            ?: "und"
    }

    private fun validatePrivateFile(raw: String, label: String): File {
        val file = validatePrivatePath(raw)
        require(file.isFile) { "The app-private $label is missing." }
        return file
    }

    private fun validatePrivatePath(raw: String): File {
        val root = File(activity.applicationInfo.dataDir).canonicalFile
        val candidate = File(raw).canonicalFile
        require(candidate.path.startsWith(root.path + File.separator)) {
            "The native document operation escaped the ostudy application sandbox."
        }
        return candidate
    }

    private fun validateToken(token: String) {
        require(token.length == 32 && token.all { it in '0'..'9' || it in 'a'..'f' }) {
            "The PDF rendering session identity is invalid."
        }
    }

    private fun closeSession(token: String) {
        sessions.remove(token)?.close()
    }

    private fun closeAllSessions() {
        val active = sessions.keys.toList()
        for (token in active) closeSession(token)
    }

    private fun safeMessage(error: Throwable): String =
        (error.message ?: error::class.java.simpleName)
            .filter { !it.isISOControl() || it == ' ' }
            .take(240)

    private data class PdfSession(
        val descriptor: ParcelFileDescriptor,
        val renderer: PdfRenderer,
        val lock: Any = Any()
    ) {
        fun close() {
            synchronized(lock) {
                runCatching { renderer.close() }
                runCatching { descriptor.close() }
            }
        }
    }
}
