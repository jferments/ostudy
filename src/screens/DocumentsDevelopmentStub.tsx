import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { InlineNotice } from "../ui/primitives/InlineNotice";

export function DocumentsDevelopmentStub() {
  return <PageScaffold data-testid="documents-development-stub">
    <PageHeader title="Documents" icon="document" />
    <InlineNotice title="Document processing is not installed in this build">
      <p>Your deck library, card authoring, practice, analytics, imports, backups, and sharing remain available. This build will not import, render, OCR, process, or delete PDFs.</p>
      <p>Provider profiles and credentials can still be configured under Settings so cloud and local-network connectivity can be tested independently.</p>
    </InlineNotice>
  </PageScaffold>;
}
