package io.github.jferments.ostudy;

import android.content.Context;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;
import android.util.Base64;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Instrumentation-only DocumentsProvider used to exercise Android's real system
 * picker and ContentResolver streaming boundary. This source is packaged only in
 * the androidTest APK and deliberately uses only Android/Java runtime classes:
 * DocumentsUI may start its provider process before AndroidJUnitRunner has
 * installed the instrumentation class-loader/runtime dependencies.
 */
public final class OstudyTestDocumentsProvider extends DocumentsProvider {
    public static final String AUTHORITY = "io.github.jferments.ostudy.dev.test.documents";
    public static final String ROOT_TITLE = "ostudy test fixtures";
    public static final String JSON_FIXTURE = "android-fixture.ostudy.json";
    public static final String IMAGE_FIXTURE = "android-fixture.png";
    public static final String PDF_FIXTURE = "android-fixture.pdf";

    private static final String ROOT_ID = "ostudy-test-root";
    private static final String FILE_PREFIX = "file:";
    private static final Set<String> FIXTURE_NAMES = new HashSet<>(
        Arrays.asList(JSON_FIXTURE, IMAGE_FIXTURE, PDF_FIXTURE)
    );
    private static final String[] ROOT_PROJECTION = new String[] {
        Root.COLUMN_ROOT_ID,
        Root.COLUMN_DOCUMENT_ID,
        Root.COLUMN_TITLE,
        Root.COLUMN_SUMMARY,
        Root.COLUMN_FLAGS,
        Root.COLUMN_MIME_TYPES,
        Root.COLUMN_AVAILABLE_BYTES,
    };
    private static final String[] DOCUMENT_PROJECTION = new String[] {
        Document.COLUMN_DOCUMENT_ID,
        Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_MIME_TYPE,
        Document.COLUMN_FLAGS,
        Document.COLUMN_SIZE,
        Document.COLUMN_LAST_MODIFIED,
    };

    @Override
    public boolean onCreate() {
        Context current = getContext();
        if (current == null) {
            return false;
        }
        try {
            seedFixtures(current);
            return true;
        } catch (IOException error) {
            throw new IllegalStateException("Unable to seed ostudy test document fixtures.", error);
        }
    }

    @Override
    public Cursor queryRoots(String[] projection) {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : ROOT_PROJECTION);
        MatrixCursor.RowBuilder row = cursor.newRow();
        row.add(Root.COLUMN_ROOT_ID, ROOT_ID);
        row.add(Root.COLUMN_DOCUMENT_ID, ROOT_ID);
        row.add(Root.COLUMN_TITLE, ROOT_TITLE);
        row.add(Root.COLUMN_SUMMARY, "ostudy instrumentation fixtures");
        row.add(Root.COLUMN_FLAGS, Root.FLAG_LOCAL_ONLY | Root.FLAG_SUPPORTS_CREATE);
        row.add(Root.COLUMN_MIME_TYPES, "*/*");
        row.add(Root.COLUMN_AVAILABLE_BYTES, fixtureDirectory(requireProviderContext()).getUsableSpace());
        return cursor;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DOCUMENT_PROJECTION);
        includeDocument(cursor, documentId);
        return cursor;
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection, String sortOrder)
        throws FileNotFoundException {
        if (!ROOT_ID.equals(parentDocumentId)) {
            throw new FileNotFoundException("Unknown test document root: " + parentDocumentId);
        }
        MatrixCursor cursor = new MatrixCursor(projection != null ? projection : DOCUMENT_PROJECTION);
        File[] children = fixtureDirectory(requireProviderContext()).listFiles();
        if (children == null) {
            return cursor;
        }
        Arrays.sort(children, Comparator.comparing(File::getName));
        for (File child : children) {
            includeDocument(cursor, fileDocumentId(child.getName()));
        }
        return cursor;
    }

    @Override
    public ParcelFileDescriptor openDocument(
        String documentId,
        String mode,
        CancellationSignal signal
    ) throws FileNotFoundException {
        return ParcelFileDescriptor.open(fileForDocument(documentId), ParcelFileDescriptor.parseMode(mode));
    }

    @Override
    public String getDocumentType(String documentId) throws FileNotFoundException {
        return ROOT_ID.equals(documentId) ? Document.MIME_TYPE_DIR : mimeType(fileForDocument(documentId));
    }

    @Override
    public String createDocument(String parentDocumentId, String mimeType, String displayName)
        throws FileNotFoundException {
        if (!ROOT_ID.equals(parentDocumentId)) {
            throw new FileNotFoundException("Files may only be created in the test fixture root.");
        }
        String safeName = sanitizeDisplayName(displayName);
        File directory = fixtureDirectory(requireProviderContext());
        File file = new File(directory, safeName);
        int suffix = 2;
        while (file.exists()) {
            int dot = safeName.lastIndexOf('.');
            String stem = dot > 0 ? safeName.substring(0, dot) : safeName;
            String extension = dot > 0 ? safeName.substring(dot) : "";
            file = new File(directory, stem + " (" + suffix + ")" + extension);
            suffix += 1;
        }
        try {
            if (!file.createNewFile()) {
                throw new IOException("Destination already exists.");
            }
        } catch (IOException error) {
            FileNotFoundException failure = new FileNotFoundException(
                "Could not create test destination " + file.getName()
            );
            failure.initCause(error);
            throw failure;
        }
        Context current = getContext();
        if (current != null) {
            current.getContentResolver().notifyChange(
                android.provider.DocumentsContract.buildChildDocumentsUri(AUTHORITY, ROOT_ID),
                null
            );
        }
        return fileDocumentId(file.getName());
    }

    @Override
    public void deleteDocument(String documentId) throws FileNotFoundException {
        File file = fileForDocument(documentId);
        if (FIXTURE_NAMES.contains(file.getName())) {
            throw new FileNotFoundException("Built-in test fixtures cannot be deleted.");
        }
        if (!file.delete()) {
            throw new FileNotFoundException("Could not delete " + file.getName());
        }
    }

    @Override
    public boolean isChildDocument(String parentDocumentId, String documentId) {
        return ROOT_ID.equals(parentDocumentId) && documentId.startsWith(FILE_PREFIX);
    }

    public static File fixtureDirectory(Context context) {
        File directory = new File(context.getFilesDir(), "documents-provider");
        if (!directory.exists() && !directory.mkdirs() && !directory.isDirectory()) {
            throw new IllegalStateException("Could not create test DocumentsProvider directory.");
        }
        return directory;
    }

    public static List<File> exportedFiles(Context context) {
        File[] files = fixtureDirectory(context).listFiles();
        List<File> exported = new ArrayList<>();
        if (files == null) {
            return exported;
        }
        for (File file : files) {
            if (file.isFile() && !FIXTURE_NAMES.contains(file.getName()) && file.length() > 0) {
                exported.add(file);
            }
        }
        exported.sort(Comparator.comparing(File::getName));
        return exported;
    }

    public static void resetExports(Context context) {
        File[] files = fixtureDirectory(context).listFiles();
        if (files == null) {
            return;
        }
        for (File file : files) {
            if (!FIXTURE_NAMES.contains(file.getName())) {
                // Best-effort cleanup; a failed deletion will be visible when a
                // later export tries to create/write its deterministic fixture.
                file.delete();
            }
        }
    }

    private void includeDocument(MatrixCursor cursor, String documentId) throws FileNotFoundException {
        if (ROOT_ID.equals(documentId)) {
            MatrixCursor.RowBuilder row = cursor.newRow();
            row.add(Document.COLUMN_DOCUMENT_ID, ROOT_ID);
            row.add(Document.COLUMN_DISPLAY_NAME, ROOT_TITLE);
            row.add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR);
            row.add(Document.COLUMN_FLAGS, Document.FLAG_DIR_SUPPORTS_CREATE);
            row.add(Document.COLUMN_SIZE, fixtureDirectory(requireProviderContext()).length());
            return;
        }
        File file = fileForDocument(documentId);
        MatrixCursor.RowBuilder row = cursor.newRow();
        row.add(Document.COLUMN_DOCUMENT_ID, documentId);
        row.add(Document.COLUMN_DISPLAY_NAME, file.getName());
        row.add(Document.COLUMN_MIME_TYPE, mimeType(file));
        row.add(
            Document.COLUMN_FLAGS,
            FIXTURE_NAMES.contains(file.getName())
                ? 0
                : Document.FLAG_SUPPORTS_WRITE | Document.FLAG_SUPPORTS_DELETE
        );
        row.add(Document.COLUMN_SIZE, file.length());
        row.add(Document.COLUMN_LAST_MODIFIED, file.lastModified());
    }

    private File fileForDocument(String documentId) throws FileNotFoundException {
        if (!documentId.startsWith(FILE_PREFIX)) {
            throw new FileNotFoundException("Unknown test document: " + documentId);
        }
        String name = documentId.substring(FILE_PREFIX.length());
        if (name.trim().isEmpty() || !name.equals(new File(name).getName())) {
            throw new FileNotFoundException("Unsafe test document ID.");
        }
        return new File(fixtureDirectory(requireProviderContext()), name);
    }

    private Context requireProviderContext() {
        Context current = getContext();
        if (current == null) {
            throw new IllegalStateException("DocumentsProvider context is unavailable.");
        }
        return current;
    }

    private static void seedFixtures(Context context) throws IOException {
        File directory = fixtureDirectory(context);
        String json = "{\n"
            + "  \"format\": \"ostudy-flashcards\",\n"
            + "  \"formatVersion\": 1,\n"
            + "  \"deck\": {\n"
            + "    \"mode\": \"create\",\n"
            + "    \"name\": \"Android DocumentsProvider Fixture\",\n"
            + "    \"description\": \"Instrumentation-only system picker fixture.\"\n"
            + "  },\n"
            + "  \"defaultTags\": [\"android-system-picker\"],\n"
            + "  \"cards\": [\n"
            + "    {\n"
            + "      \"type\": \"basic\",\n"
            + "      \"front\": \"Which Android boundary selected this fixture?\",\n"
            + "      \"back\": \"Storage Access Framework\",\n"
            + "      \"reverseEnabled\": false,\n"
            + "      \"tags\": [\"android\"]\n"
            + "    }\n"
            + "  ]\n"
            + "}\n";
        writeBytes(
            new File(directory, JSON_FIXTURE),
            json.getBytes(StandardCharsets.UTF_8)
        );
        writeBytes(
            new File(directory, IMAGE_FIXTURE),
            Base64.decode(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9WlSmOAAAAAASUVORK5CYII=",
                Base64.DEFAULT
            )
        );
        writeBytes(
            new File(directory, PDF_FIXTURE),
            ("%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n"
                + "trailer\n<< /Root 1 0 R >>\n%%EOF\n").getBytes(StandardCharsets.UTF_8)
        );
    }

    private static void writeBytes(File file, byte[] bytes) throws IOException {
        try (FileOutputStream output = new FileOutputStream(file, false)) {
            output.write(bytes);
        }
    }

    private static String fileDocumentId(String name) {
        return FILE_PREFIX + name;
    }

    private static String sanitizeDisplayName(String value) {
        String sanitized = value.replaceAll("[^A-Za-z0-9._ ()-]", "_").trim();
        return sanitized.isEmpty() ? "ostudy-export.bin" : sanitized;
    }

    private static String mimeType(File file) {
        String name = file.getName();
        if (name.endsWith(".ostudy.json")) return "application/json";
        if (name.endsWith(".png")) return "image/png";
        if (name.endsWith(".pdf")) return "application/pdf";
        return "application/octet-stream";
    }
}
