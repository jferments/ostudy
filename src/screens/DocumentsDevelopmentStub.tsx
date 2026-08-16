import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import styles from "./DocumentsDevelopmentStub.module.css";

export function DocumentsDevelopmentStub() {
  return (
    <section className={styles.page} data-testid="documents-development-stub">
      <PageHeader title="Documents" icon="document" compact />

      <div className={styles.panel}>
        <div aria-hidden="true" className={styles.icon}>▣</div>
        <div>
          <h2>Document processing is not installed in this build</h2>
          <p>
            Your deck library, card authoring, practice, analytics, imports, backups, and sharing
            remain available. This build will not import, render, OCR, process, or delete PDFs.
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
