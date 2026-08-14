import styles from "./DocumentsDevelopmentStub.module.css";

export function DocumentsDevelopmentStub() {
  return (
    <section className={styles.page} data-testid="documents-development-stub">
      <header className={styles.header}>
        <p className={styles.eyebrow}>ANDROID DEVELOPMENT PREVIEW</p>
        <h1>Documents and AI generation</h1>
        <p>
          Your deck library, card authoring, practice, analytics, imports, backups, and sharing
          work in this Android build. PDF extraction, OCR, and AI-generated flashcards will be
          added in the next Android development phase.
        </p>
      </header>

      <div className={styles.panel}>
        <div aria-hidden="true" className={styles.icon}>▣</div>
        <div>
          <h2>Document processing is not installed yet</h2>
          <p>
            ostudy will preserve document records and source files restored from a complete
            backup, but this build will not import, render, OCR, process, or delete PDFs.
          </p>
          <p>
            Provider profiles and credentials can still be configured under Settings so cloud
            and local-network connectivity can be tested independently.
          </p>
        </div>
      </div>
    </section>
  );
}
