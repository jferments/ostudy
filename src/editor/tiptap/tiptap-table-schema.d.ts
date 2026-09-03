import "@tiptap/core";

declare module "@tiptap/core" {
  interface NodeConfig<Options, Storage> {
    /** ProseMirror table schema role consumed by @tiptap/pm/tables commands. */
    tableRole?: "table" | "row" | "cell" | "header_cell";
  }
}
