package io.github.jferments.ostudy.platform

import android.app.Activity
import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.Uri
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import android.provider.OpenableColumns
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.Logger
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.security.KeyStore
import java.security.MessageDigest
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.UUID
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val SECRET_ALIAS = "io.github.jferments.ostudy.ai-secrets.v1"
private const val SECRET_PREFERENCES = "ostudy-secure-secrets-v1"
private const val INCOMING_PREFERENCES = "ostudy-incoming-shares-v1"
private const val MAX_SECRET_BYTES = 64 * 1024
private const val MAX_INCOMING_BYTES = 8L * 1024L * 1024L * 1024L
private const val COPY_BUFFER_BYTES = 1024 * 1024

@InvokeArg
class TimestampArgs {
    var utcMillis: Long = 0
}

@InvokeArg
class DateArgs {
    lateinit var date: String
}

@InvokeArg
class AcquireImportArgs {
    lateinit var title: String
    var mimeTypes: Array<String> = emptyArray()
    lateinit var targetPath: String
    var maximumBytes: Long = 0
}

@InvokeArg
class PublishExportArgs {
    lateinit var title: String
    lateinit var suggestedFilename: String
    lateinit var mimeType: String
    lateinit var sourcePath: String
    var maximumBytes: Long = 0
}

@InvokeArg
class ShareFileArgs {
    lateinit var title: String
    lateinit var displayName: String
    lateinit var mimeType: String
    lateinit var sourcePath: String
}

@InvokeArg
class BackInterceptionArgs {
    var enabled: Boolean = false
}

@InvokeArg
class SecretArgs {
    lateinit var locator: String
}

@InvokeArg
class WriteSecretArgs {
    lateinit var locator: String
    lateinit var valueBase64: String
}

@InvokeArg
class IncomingShareArgs {
    lateinit var token: String
}

@TauriPlugin
class OstudyPlatformPlugin(private val activity: Activity) : Plugin(activity) {
    private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val incomingPreferences =
        activity.getSharedPreferences(INCOMING_PREFERENCES, Context.MODE_PRIVATE)
    private val secretPreferences =
        activity.getSharedPreferences(SECRET_PREFERENCES, Context.MODE_PRIVATE)

    @Volatile private var pendingImport: AcquireImportArgs? = null
    @Volatile private var pendingExport: PublishExportArgs? = null
    private var backCallback: OnBackPressedCallback? = null
    private var lifecycleObserver: DefaultLifecycleObserver? = null
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    override fun load(webView: WebView) {
        installBackDispatcher()
        installLifecycleObserver()
        installInsetsObserver(webView)
        installNetworkObserver()
        processIncomingIntent(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        processIncomingIntent(intent)
    }

    @Command
    fun getEnvironment(invoke: Invoke) {
        invoke.resolve(environmentObject())
    }

    @Command
    fun timeZoneSnapshot(invoke: Invoke) {
        val args = invoke.parseArgs(TimestampArgs::class.java)
        try {
            val instant = Instant.ofEpochMilli(args.utcMillis)
            val zone = ZoneId.systemDefault()
            val result = JSObject()
            result.put("identifier", zone.id)
            result.put("utcOffsetMinutes", zone.rules.getOffset(instant).totalSeconds / 60)
            invoke.resolve(result)
        } catch (error: Exception) {
            invoke.reject("The Android time-zone snapshot could not be calculated: ${safeMessage(error)}")
        }
    }

    @Command
    fun localDateAt(invoke: Invoke) {
        val args = invoke.parseArgs(TimestampArgs::class.java)
        try {
            val result = JSObject()
            result.put(
                "date",
                Instant.ofEpochMilli(args.utcMillis).atZone(ZoneId.systemDefault()).toLocalDate().toString()
            )
            invoke.resolve(result)
        } catch (error: Exception) {
            invoke.reject("The Android local date could not be calculated: ${safeMessage(error)}")
        }
    }

    @Command
    fun firstValidInstant(invoke: Invoke) {
        val args = invoke.parseArgs(DateArgs::class.java)
        try {
            val result = JSObject()
            result.put(
                "utcMillis",
                LocalDate.parse(args.date).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
            )
            invoke.resolve(result)
        } catch (error: Exception) {
            invoke.reject("The Android study-day boundary could not be calculated: ${safeMessage(error)}")
        }
    }

    @Command
    fun acquireImport(invoke: Invoke) {
        if (pendingImport != null || pendingExport != null) {
            invoke.reject("Another Android document operation is already open.")
            return
        }
        val args = invoke.parseArgs(AcquireImportArgs::class.java)
        try {
            validatePrivatePath(args.targetPath)
            require(args.maximumBytes in 1..MAX_INCOMING_BYTES) { "The import size limit is invalid." }
            pendingImport = args
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = if (args.mimeTypes.size == 1) args.mimeTypes.first() else "*/*"
                if (args.mimeTypes.size > 1) {
                    putExtra(Intent.EXTRA_MIME_TYPES, args.mimeTypes)
                }
            }
            startActivityForResult(invoke, intent, "acquireImportResult")
        } catch (error: Exception) {
            pendingImport = null
            invoke.reject("The Android document picker could not open: ${safeMessage(error)}")
        }
    }

    @ActivityCallback
    fun acquireImportResult(invoke: Invoke, result: ActivityResult) {
        val args = pendingImport
        pendingImport = null
        if (args == null) {
            invoke.reject("The Android document-picker state was lost.")
            return
        }
        if (result.resultCode == Activity.RESULT_CANCELED) {
            invoke.resolve(cancelledTransfer())
            return
        }
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.reject("The Android document picker did not return a readable file.")
            return
        }
        ioScope.launch {
            try {
                val copied = copyUriToPrivateFile(uri, File(args.targetPath), args.maximumBytes)
                val response = JSObject()
                response.put("cancelled", false)
                response.put("displayName", queryDisplayName(uri) ?: "selected file")
                response.put("reportedMediaType", activity.contentResolver.getType(uri))
                response.put("copiedBytes", copied)
                withContext(Dispatchers.Main) { invoke.resolve(response) }
            } catch (error: Exception) {
                File(args.targetPath).delete()
                withContext(Dispatchers.Main) {
                    invoke.reject("The selected Android document could not be copied: ${safeMessage(error)}")
                }
            }
        }
    }

    @Command
    fun publishExport(invoke: Invoke) {
        if (pendingImport != null || pendingExport != null) {
            invoke.reject("Another Android document operation is already open.")
            return
        }
        val args = invoke.parseArgs(PublishExportArgs::class.java)
        try {
            val source = validatePrivatePath(args.sourcePath)
            require(source.isFile) { "The prepared export is missing." }
            require(args.maximumBytes > 0) { "The export size limit is invalid." }
            pendingExport = args
            val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = args.mimeType.ifBlank { "application/octet-stream" }
                putExtra(Intent.EXTRA_TITLE, args.suggestedFilename)
            }
            startActivityForResult(invoke, intent, "publishExportResult")
        } catch (error: Exception) {
            pendingExport = null
            invoke.reject("The Android save dialog could not open: ${safeMessage(error)}")
        }
    }

    @ActivityCallback
    fun publishExportResult(invoke: Invoke, result: ActivityResult) {
        val args = pendingExport
        pendingExport = null
        if (args == null) {
            invoke.reject("The Android save-dialog state was lost.")
            return
        }
        if (result.resultCode == Activity.RESULT_CANCELED) {
            invoke.resolve(cancelledTransfer())
            return
        }
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.reject("The Android document provider did not return a writable destination.")
            return
        }
        ioScope.launch {
            try {
                val source = validatePrivatePath(args.sourcePath)
                val copied = copyPrivateFileToUri(source, uri, args.maximumBytes)
                val response = JSObject()
                response.put("cancelled", false)
                response.put("displayName", queryDisplayName(uri) ?: args.suggestedFilename)
                response.put("copiedBytes", copied)
                withContext(Dispatchers.Main) { invoke.resolve(response) }
            } catch (error: Exception) {
                withContext(Dispatchers.Main) {
                    invoke.reject("The prepared export could not be written: ${safeMessage(error)}")
                }
            }
        }
    }

    @Command
    fun shareFile(invoke: Invoke) {
        val args = invoke.parseArgs(ShareFileArgs::class.java)
        try {
            val source = validatePrivatePath(args.sourcePath)
            require(source.isFile) { "The prepared share file is missing." }
            require(source.canonicalPath.contains("${File.separator}transfers${File.separator}exports${File.separator}")) {
                "Only a prepared ostudy export can be shared."
            }
            val uri = FileProvider.getUriForFile(
                activity,
                "${activity.packageName}.fileprovider",
                source
            )
            val share = Intent(Intent.ACTION_SEND).apply {
                type = args.mimeType.ifBlank { "application/octet-stream" }
                putExtra(Intent.EXTRA_STREAM, uri)
                clipData = android.content.ClipData.newRawUri(args.displayName, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(Intent.createChooser(share, args.title))
            val response = JSObject()
            response.put("launched", true)
            invoke.resolve(response)
        } catch (error: Exception) {
            invoke.reject("The Android Sharesheet could not open: ${safeMessage(error)}")
        }
    }

    @Command
    fun setBackInterception(invoke: Invoke) {
        val args = invoke.parseArgs(BackInterceptionArgs::class.java)
        activity.runOnUiThread {
            backCallback?.isEnabled = args.enabled
            invoke.resolve()
        }
    }

    @Command
    fun secretAvailability(invoke: Invoke) {
        val response = JSObject()
        response.put("state", try {
            getOrCreateSecretKey()
            "available"
        } catch (_: Exception) {
            "unavailable"
        })
        invoke.resolve(response)
    }

    @Command
    fun writeSecret(invoke: Invoke) {
        val args = invoke.parseArgs(WriteSecretArgs::class.java)
        ioScope.launch {
            try {
                validateLocator(args.locator)
                val plaintext = Base64.decode(args.valueBase64, Base64.NO_WRAP)
                require(plaintext.isNotEmpty() && plaintext.size <= MAX_SECRET_BYTES) {
                    "The credential has an unsupported size."
                }
                val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                cipher.init(Cipher.ENCRYPT_MODE, getOrCreateSecretKey())
                val ciphertext = cipher.doFinal(plaintext)
                plaintext.fill(0)
                val envelope = ByteArray(cipher.iv.size + ciphertext.size)
                System.arraycopy(cipher.iv, 0, envelope, 0, cipher.iv.size)
                System.arraycopy(ciphertext, 0, envelope, cipher.iv.size, ciphertext.size)
                secretPreferences.edit()
                    .putString(secretPreferenceKey(args.locator), Base64.encodeToString(envelope, Base64.NO_WRAP))
                    .commit()
                envelope.fill(0)
                withContext(Dispatchers.Main) { invoke.resolve() }
            } catch (error: Exception) {
                withContext(Dispatchers.Main) {
                    invoke.reject("The Android secure store could not save this credential: ${safeMessage(error)}")
                }
            }
        }
    }

    @Command
    fun readSecret(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        ioScope.launch {
            val response = JSObject()
            try {
                validateLocator(args.locator)
                val encoded = secretPreferences.getString(secretPreferenceKey(args.locator), null)
                if (encoded == null) {
                    response.put("state", "missing")
                    response.put("valueBase64", null)
                } else {
                    val envelope = Base64.decode(encoded, Base64.NO_WRAP)
                    require(envelope.size > 12) { "The encrypted credential record is invalid." }
                    val nonce = envelope.copyOfRange(0, 12)
                    val ciphertext = envelope.copyOfRange(12, envelope.size)
                    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                    cipher.init(Cipher.DECRYPT_MODE, getOrCreateSecretKey(), GCMParameterSpec(128, nonce))
                    val plaintext = cipher.doFinal(ciphertext)
                    response.put("state", "available")
                    response.put("valueBase64", Base64.encodeToString(plaintext, Base64.NO_WRAP))
                    plaintext.fill(0)
                    envelope.fill(0)
                }
            } catch (_: KeyPermanentlyInvalidatedException) {
                response.put("state", "invalidated")
                response.put("valueBase64", null)
            } catch (_: AEADBadTagException) {
                response.put("state", "invalidated")
                response.put("valueBase64", null)
            } catch (error: Exception) {
                Logger.error("Android secure-store read failed: ${safeMessage(error)}")
                response.put("state", "failed")
                response.put("valueBase64", null)
            }
            withContext(Dispatchers.Main) { invoke.resolve(response) }
        }
    }

    @Command
    fun deleteSecret(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        try {
            validateLocator(args.locator)
            secretPreferences.edit().remove(secretPreferenceKey(args.locator)).commit()
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("The Android secure store could not remove this credential: ${safeMessage(error)}")
        }
    }

    @Command
    fun listIncomingShares(invoke: Invoke) {
        val result = JSArray()
        for ((token, raw) in incomingPreferences.all) {
            val encoded = raw as? String ?: continue
            val metadata = parseIncomingMetadata(encoded) ?: continue
            val file = incomingFile(token)
            if (!file.isFile || file.length() != metadata.byteSize) {
                removeIncomingShare(token)
                continue
            }
            result.put(incomingSummaryObject(token, metadata))
        }
        invoke.resolve(result)
    }

    @Command
    fun takeIncomingShare(invoke: Invoke) {
        val args = invoke.parseArgs(IncomingShareArgs::class.java)
        try {
            validateToken(args.token)
            val raw = incomingPreferences.getString(args.token, null)
                ?: throw IllegalArgumentException("That incoming share has expired.")
            val metadata = parseIncomingMetadata(raw)
                ?: throw IllegalArgumentException("That incoming share is invalid.")
            val file = incomingFile(args.token)
            require(file.isFile && file.length() == metadata.byteSize) {
                "The incoming shared file is missing."
            }
            val response = incomingSummaryObject(args.token, metadata)
            response.put("path", file.canonicalPath)
            invoke.resolve(response)
        } catch (error: Exception) {
            invoke.reject("The incoming Android share could not be opened: ${safeMessage(error)}")
        }
    }

    @Command
    fun dismissIncomingShare(invoke: Invoke) {
        val args = invoke.parseArgs(IncomingShareArgs::class.java)
        try {
            validateToken(args.token)
            removeIncomingShare(args.token)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("The incoming Android share could not be dismissed: ${safeMessage(error)}")
        }
    }

    private fun installBackDispatcher() {
        val component = activity as? ComponentActivity ?: return
        val callback = object : OnBackPressedCallback(false) {
            override fun handleOnBackPressed() {
                trigger("back", JSObject())
            }
        }
        component.onBackPressedDispatcher.addCallback(component, callback)
        backCallback = callback
    }

    private fun installLifecycleObserver() {
        val owner = activity as? LifecycleOwner ?: return
        val observer = object : DefaultLifecycleObserver {
            override fun onResume(owner: LifecycleOwner) {
                emitLifecycle("resuming")
                emitLifecycle("foreground")
                publishEnvironment()
            }
            override fun onPause(owner: LifecycleOwner) = emitLifecycle("background")
            override fun onStop(owner: LifecycleOwner) = emitLifecycle("suspending")
            override fun onDestroy(owner: LifecycleOwner) {
                if (activity.isFinishing) emitLifecycle("terminating")
                ioScope.cancel()
            }
        }
        owner.lifecycle.addObserver(observer)
        lifecycleObserver = observer
        emitLifecycle("foreground")
    }

    private fun installInsetsObserver(webView: WebView) {
        ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
            val system = maxInsets(
                insets.getInsets(WindowInsetsCompat.Type.systemBars()),
                insets.getInsets(WindowInsetsCompat.Type.displayCutout())
            )
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val response = JSObject()
            response.put("top", system.top)
            response.put("right", system.right)
            response.put("bottom", system.bottom)
            response.put("left", system.left)
            response.put("imeBottom", ime.bottom)
            response.put("imeVisible", insets.isVisible(WindowInsetsCompat.Type.ime()))
            trigger("insets", response)
            insets
        }
        ViewCompat.requestApplyInsets(webView)
    }

    private fun installNetworkObserver() {
        val connectivity = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) = emitNetwork()
            override fun onLost(network: Network) = emitNetwork()
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = emitNetwork()
        }
        try {
            connectivity.registerDefaultNetworkCallback(callback)
            networkCallback = callback
        } catch (error: Exception) {
            Logger.warn("Android network observation is unavailable: ${safeMessage(error)}")
        }
    }

    private fun emitLifecycle(phase: String) {
        val payload = JSObject()
        payload.put("schemaVersion", 1)
        payload.put("phase", phase)
        payload.put("observedAtUtcMillis", System.currentTimeMillis())
        trigger("lifecycle", payload)
    }

    private fun emitNetwork() {
        trigger("network", networkObject())
    }

    private fun publishEnvironment() {
        trigger("environment", environmentObject())
    }

    private fun environmentObject(): JSObject {
        val zone = ZoneId.systemDefault()
        val now = Instant.now()
        val activityManager = activity.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val memory = ActivityManager.MemoryInfo().also(activityManager::getMemoryInfo)
        val power = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        val response = networkObject()
        response.put("timeZoneId", zone.id)
        response.put("utcOffsetMinutes", zone.rules.getOffset(now).totalSeconds / 60)
        response.put("logicalCpuCount", Runtime.getRuntime().availableProcessors().coerceAtLeast(1))
        response.put("availableMemoryBytes", memory.availMem)
        response.put("lowMemory", memory.lowMemory)
        response.put("thermalState", thermalState(power))
        response.put("batterySaver", power.isPowerSaveMode)
        response.put("charging", chargingState())
        return response
    }

    private fun networkObject(): JSObject {
        val connectivity = activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val active = connectivity.activeNetwork
        val capabilities = active?.let(connectivity::getNetworkCapabilities)
        val response = JSObject()
        response.put("online", active != null)
        response.put("metered", connectivity.isActiveNetworkMetered)
        response.put(
            "validatedInternet",
            capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED) == true
        )
        response.put("transport", transportName(capabilities))
        return response
    }

    private fun processIncomingIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND) return
        val uri = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(Intent.EXTRA_STREAM) as? Uri
        } ?: return
        ioScope.launch {
            try {
                val token = UUID.randomUUID().toString().replace("-", "")
                val target = incomingFile(token)
                val copied = copyUriToPrivateFile(uri, target, MAX_INCOMING_BYTES)
                val metadata = IncomingMetadata(
                    displayName = queryDisplayName(uri) ?: "shared ostudy file",
                    mediaType = activity.contentResolver.getType(uri),
                    byteSize = copied
                )
                incomingPreferences.edit().putString(token, metadata.toJson()).commit()
                val payload = incomingSummaryObject(token, metadata)
                withContext(Dispatchers.Main) { trigger("incoming-share", payload) }
            } catch (error: Exception) {
                Logger.error("Incoming Android share could not be cached: ${safeMessage(error)}")
            }
        }
    }

    private fun copyUriToPrivateFile(uri: Uri, target: File, maximumBytes: Long): Long {
        validatePrivatePath(target.path)
        target.parentFile?.mkdirs()
        val input = activity.contentResolver.openInputStream(uri)
            ?: throw IllegalArgumentException("The document provider returned no input stream.")
        var total = 0L
        try {
            input.use { source ->
                FileOutputStream(target, false).use { output ->
                    val buffer = ByteArray(COPY_BUFFER_BYTES)
                    while (true) {
                        val count = source.read(buffer)
                        if (count < 0) break
                        total += count
                        require(total <= maximumBytes) { "The selected file exceeds the supported size limit." }
                        output.write(buffer, 0, count)
                    }
                    output.fd.sync()
                }
            }
            return total
        } catch (error: Exception) {
            target.delete()
            throw error
        }
    }

    private fun copyPrivateFileToUri(source: File, uri: Uri, maximumBytes: Long): Long {
        require(source.length() <= maximumBytes) { "The prepared export exceeds its supported size limit." }
        val output = activity.contentResolver.openOutputStream(uri, "w")
            ?: throw IllegalArgumentException("The document provider returned no output stream.")
        var total = 0L
        FileInputStream(source).use { input ->
            output.use { destination ->
                val buffer = ByteArray(COPY_BUFFER_BYTES)
                while (true) {
                    val count = input.read(buffer)
                    if (count < 0) break
                    total += count
                    require(total <= maximumBytes) { "The prepared export changed while it was being published." }
                    destination.write(buffer, 0, count)
                }
                destination.flush()
            }
        }
        return total
    }

    private fun queryDisplayName(uri: Uri): String? {
        activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (index >= 0) return cursor.getString(index)?.take(255)
            }
        }
        return null
    }

    private fun validatePrivatePath(raw: String): File {
        val root = File(activity.applicationInfo.dataDir).canonicalFile
        val candidate = File(raw).canonicalFile
        require(candidate.path.startsWith(root.path + File.separator)) {
            "The native file operation escaped the ostudy application sandbox."
        }
        return candidate
    }

    private fun getOrCreateSecretKey(): SecretKey {
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keyStore.getKey(SECRET_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        val specification = KeyGenParameterSpec.Builder(
            SECRET_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        )
            .setKeySize(256)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true)
            .build()
        generator.init(specification)
        return generator.generateKey()
    }

    private fun secretPreferenceKey(locator: String): String {
        validateLocator(locator)
        return MessageDigest.getInstance("SHA-256")
            .digest(locator.toByteArray(Charsets.UTF_8))
            .joinToString("") { "%02x".format(it) }
    }

    private fun validateLocator(locator: String) {
        require(locator.isNotBlank() && locator.length <= 512 && locator.none(Char::isISOControl)) {
            "The secure-store locator is invalid."
        }
    }

    private fun incomingFile(token: String): File {
        validateToken(token)
        return File(File(activity.cacheDir, "ostudy-incoming-shares"), "$token.incoming")
    }

    private fun validateToken(token: String) {
        require(token.length == 32 && token.all { it in '0'..'9' || it in 'a'..'f' }) {
            "The incoming-share identity is invalid."
        }
    }

    private fun removeIncomingShare(token: String) {
        incomingPreferences.edit().remove(token).apply()
        runCatching { incomingFile(token).delete() }
    }

    private fun incomingSummaryObject(token: String, metadata: IncomingMetadata): JSObject {
        val result = JSObject()
        result.put("token", token)
        result.put("displayName", metadata.displayName)
        result.put("reportedMediaType", metadata.mediaType)
        result.put("byteSize", metadata.byteSize)
        return result
    }

    private fun parseIncomingMetadata(raw: String): IncomingMetadata? = try {
        val value = JSONObject(raw)
        IncomingMetadata(
            displayName = value.getString("displayName").take(255),
            mediaType = value.optString("mediaType").takeIf(String::isNotBlank),
            byteSize = value.getLong("byteSize")
        )
    } catch (_: Exception) {
        null
    }

    private fun cancelledTransfer(): JSObject = JSObject().apply {
        put("cancelled", true)
        put("displayName", null)
        put("reportedMediaType", null)
        put("copiedBytes", 0)
    }

    private fun chargingState(): Boolean? {
        val battery = activity.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED)) ?: return null
        val status = battery.getIntExtra(BatteryManager.EXTRA_STATUS, -1)
        return status == BatteryManager.BATTERY_STATUS_CHARGING ||
            status == BatteryManager.BATTERY_STATUS_FULL
    }

    private fun thermalState(power: PowerManager): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return "unknown"
        return when (power.currentThermalStatus) {
            PowerManager.THERMAL_STATUS_NONE, PowerManager.THERMAL_STATUS_LIGHT -> "nominal"
            PowerManager.THERMAL_STATUS_MODERATE -> "fair"
            PowerManager.THERMAL_STATUS_SEVERE -> "serious"
            PowerManager.THERMAL_STATUS_CRITICAL,
            PowerManager.THERMAL_STATUS_EMERGENCY,
            PowerManager.THERMAL_STATUS_SHUTDOWN -> "critical"
            else -> "unknown"
        }
    }

    private fun transportName(capabilities: NetworkCapabilities?): String = when {
        capabilities == null -> "none"
        capabilities.hasTransport(NetworkCapabilities.TRANSPORT_VPN) -> "vpn"
        capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> "wifi"
        capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> "cellular"
        capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "ethernet"
        else -> "other"
    }

    private fun maxInsets(first: Insets, second: Insets): Insets = Insets.of(
        maxOf(first.left, second.left),
        maxOf(first.top, second.top),
        maxOf(first.right, second.right),
        maxOf(first.bottom, second.bottom)
    )

    private fun safeMessage(error: Throwable): String =
        (error.message ?: error::class.java.simpleName)
            .filter { !it.isISOControl() || it == ' ' }
            .take(240)

    private data class IncomingMetadata(
        val displayName: String,
        val mediaType: String?,
        val byteSize: Long
    ) {
        fun toJson(): String = JSONObject()
            .put("displayName", displayName)
            .put("mediaType", mediaType)
            .put("byteSize", byteSize)
            .toString()
    }
}
