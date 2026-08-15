package io.github.jferments.ostudy

import android.content.Context
import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import android.util.Base64
import java.io.File

/**
 * Instrumentation-only DocumentsProvider used to exercise Android's real system
 * picker and ContentResolver streaming boundary. This source is packaged only in
 * the androidTest APK and is never present in application or release packages.
 */
class OstudyTestDocumentsProvider : DocumentsProvider() {
    override fun onCreate(): Boolean {
        val current = context ?: return false
        seedFixtures(current)
        return true
    }

    override fun queryRoots(projection: Array<out String>?): Cursor {
        val cursor = MatrixCursor(projection ?: ROOT_PROJECTION)
        cursor.newRow().apply {
            add(Root.COLUMN_ROOT_ID, ROOT_ID)
            add(Root.COLUMN_DOCUMENT_ID, ROOT_ID)
            add(Root.COLUMN_TITLE, ROOT_TITLE)
            add(Root.COLUMN_SUMMARY, "ostudy instrumentation fixtures")
            add(Root.COLUMN_FLAGS, Root.FLAG_LOCAL_ONLY or Root.FLAG_SUPPORTS_CREATE)
            add(Root.COLUMN_MIME_TYPES, "*/*")
            add(Root.COLUMN_AVAILABLE_BYTES, fixtureDirectory(requireContext()).usableSpace)
        }
        return cursor
    }

    override fun queryDocument(documentId: String, projection: Array<out String>?): Cursor {
        val cursor = MatrixCursor(projection ?: DOCUMENT_PROJECTION)
        includeDocument(cursor, documentId)
        return cursor
    }

    override fun queryChildDocuments(
        parentDocumentId: String,
        projection: Array<out String>?,
        sortOrder: String?
    ): Cursor {
        require(parentDocumentId == ROOT_ID) { "Unknown test document root: $parentDocumentId" }
        val cursor = MatrixCursor(projection ?: DOCUMENT_PROJECTION)
        fixtureDirectory(requireContext())
            .listFiles()
            .orEmpty()
            .sortedBy { it.name }
            .forEach { includeDocument(cursor, fileDocumentId(it.name)) }
        return cursor
    }

    override fun openDocument(
        documentId: String,
        mode: String,
        signal: CancellationSignal?
    ): ParcelFileDescriptor {
        val file = fileForDocument(documentId)
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.parseMode(mode))
    }

    override fun getDocumentType(documentId: String): String =
        if (documentId == ROOT_ID) Document.MIME_TYPE_DIR else mimeType(fileForDocument(documentId))

    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String {
        require(parentDocumentId == ROOT_ID) { "Files may only be created in the test fixture root." }
        val safeName = sanitizeDisplayName(displayName)
        val directory = fixtureDirectory(requireContext())
        var file = File(directory, safeName)
        var suffix = 2
        while (file.exists()) {
            val dot = safeName.lastIndexOf('.')
            val stem = if (dot > 0) safeName.substring(0, dot) else safeName
            val extension = if (dot > 0) safeName.substring(dot) else ""
            file = File(directory, "$stem ($suffix)$extension")
            suffix += 1
        }
        file.createNewFile()
        context?.contentResolver?.notifyChange(
            android.provider.DocumentsContract.buildChildDocumentsUri(AUTHORITY, ROOT_ID),
            null
        )
        return fileDocumentId(file.name)
    }

    override fun deleteDocument(documentId: String) {
        val file = fileForDocument(documentId)
        require(file.name !in FIXTURE_NAMES) { "Built-in test fixtures cannot be deleted." }
        if (!file.delete()) throw IllegalStateException("Could not delete ${file.name}")
    }

    override fun isChildDocument(parentDocumentId: String, documentId: String): Boolean =
        parentDocumentId == ROOT_ID && documentId.startsWith(FILE_PREFIX)

    private fun includeDocument(cursor: MatrixCursor, documentId: String) {
        if (documentId == ROOT_ID) {
            cursor.newRow().apply {
                add(Document.COLUMN_DOCUMENT_ID, ROOT_ID)
                add(Document.COLUMN_DISPLAY_NAME, ROOT_TITLE)
                add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR)
                add(Document.COLUMN_FLAGS, Document.FLAG_DIR_SUPPORTS_CREATE)
                add(Document.COLUMN_SIZE, fixtureDirectory(requireContext()).length())
            }
            return
        }
        val file = fileForDocument(documentId)
        cursor.newRow().apply {
            add(Document.COLUMN_DOCUMENT_ID, documentId)
            add(Document.COLUMN_DISPLAY_NAME, file.name)
            add(Document.COLUMN_MIME_TYPE, mimeType(file))
            add(
                Document.COLUMN_FLAGS,
                if (file.name in FIXTURE_NAMES) 0
                else Document.FLAG_SUPPORTS_WRITE or Document.FLAG_SUPPORTS_DELETE
            )
            add(Document.COLUMN_SIZE, file.length())
            add(Document.COLUMN_LAST_MODIFIED, file.lastModified())
        }
    }

    private fun fileForDocument(documentId: String): File {
        require(documentId.startsWith(FILE_PREFIX)) { "Unknown test document: $documentId" }
        val name = documentId.removePrefix(FILE_PREFIX)
        require(name.isNotBlank() && name == File(name).name) { "Unsafe test document ID." }
        return File(fixtureDirectory(requireContext()), name)
    }

    private fun requireContext(): Context = requireNotNull(context) { "DocumentsProvider context is unavailable." }

    companion object {
        const val AUTHORITY = "io.github.jferments.ostudy.dev.test.documents"
        const val ROOT_TITLE = "ostudy test fixtures"
        const val JSON_FIXTURE = "android-fixture.ostudy.json"
        const val IMAGE_FIXTURE = "android-fixture.png"
        const val PDF_FIXTURE = "android-fixture.pdf"

        private const val ROOT_ID = "ostudy-test-root"
        private const val FILE_PREFIX = "file:"
        private val FIXTURE_NAMES = setOf(JSON_FIXTURE, IMAGE_FIXTURE, PDF_FIXTURE)
        private val ROOT_PROJECTION = arrayOf(
            Root.COLUMN_ROOT_ID,
            Root.COLUMN_DOCUMENT_ID,
            Root.COLUMN_TITLE,
            Root.COLUMN_SUMMARY,
            Root.COLUMN_FLAGS,
            Root.COLUMN_MIME_TYPES,
            Root.COLUMN_AVAILABLE_BYTES
        )
        private val DOCUMENT_PROJECTION = arrayOf(
            Document.COLUMN_DOCUMENT_ID,
            Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE,
            Document.COLUMN_FLAGS,
            Document.COLUMN_SIZE,
            Document.COLUMN_LAST_MODIFIED
        )

        fun fixtureDirectory(context: Context): File =
            File(context.filesDir, "documents-provider").apply { mkdirs() }

        fun exportedFiles(context: Context): List<File> =
            fixtureDirectory(context).listFiles().orEmpty()
                .filter { it.isFile && it.name !in FIXTURE_NAMES && it.length() > 0 }
                .sortedBy { it.name }

        fun resetExports(context: Context) {
            fixtureDirectory(context).listFiles().orEmpty()
                .filter { it.name !in FIXTURE_NAMES }
                .forEach(File::delete)
        }

        private fun seedFixtures(context: Context) {
            val directory = fixtureDirectory(context)
            File(directory, JSON_FIXTURE).writeText(
                """
                {
                  "format": "ostudy-flashcards",
                  "formatVersion": 1,
                  "deck": {
                    "mode": "create",
                    "name": "Android DocumentsProvider Fixture",
                    "description": "Instrumentation-only system picker fixture."
                  },
                  "defaultTags": ["android-system-picker"],
                  "cards": [
                    {
                      "type": "basic",
                      "front": "Which Android boundary selected this fixture?",
                      "back": "Storage Access Framework",
                      "reverseEnabled": false,
                      "tags": ["android"]
                    }
                  ]
                }
                """.trimIndent() + "\n"
            )
            File(directory, IMAGE_FIXTURE).writeBytes(
                Base64.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9WlSmOAAAAAASUVORK5CYII=",
                    Base64.DEFAULT
                )
            )
            File(directory, PDF_FIXTURE).writeText(
                "%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"
            )
        }

        private fun fileDocumentId(name: String): String = "$FILE_PREFIX$name"

        private fun sanitizeDisplayName(value: String): String =
            value.replace(Regex("[^A-Za-z0-9._ ()-]"), "_").trim().ifBlank { "ostudy-export.bin" }

        private fun mimeType(file: File): String = when {
            file.name.endsWith(".ostudy.json") -> "application/json"
            file.name.endsWith(".png") -> "image/png"
            file.name.endsWith(".pdf") -> "application/pdf"
            file.name.endsWith(".studydeck") -> "application/octet-stream"
            file.name.endsWith(".ostudybackup") -> "application/octet-stream"
            else -> "application/octet-stream"
        }
    }
}
