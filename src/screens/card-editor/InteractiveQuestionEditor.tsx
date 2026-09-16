import type { Asset } from "../../ipc/assets";
import { ChoiceQuestionEditor } from "./ChoiceQuestionEditor";
import { GroupingQuestionEditor } from "./GroupingQuestionEditor";
import { MatchingQuestionEditor } from "./MatchingQuestionEditor";
import { OrderingQuestionEditor } from "./OrderingQuestionEditor";
import { TrueFalseQuestionEditor } from "./TrueFalseQuestionEditor";
import type { QuestionEditorDraft } from "./questionDraft";

interface InteractiveQuestionEditorProps {
  draft: QuestionEditorDraft;
  assets: Asset[];
  assetDraftId: string | null;
  disabled: boolean;
  onAssetImported: (asset: Asset) => void;
  onChange: (draft: QuestionEditorDraft) => void;
}

export function InteractiveQuestionEditor({
  draft,
  assets,
  assetDraftId,
  disabled,
  onAssetImported,
  onChange,
}: InteractiveQuestionEditorProps) {
  const shared = { assets, assetDraftId, disabled, onAssetImported };
  switch (draft.kind) {
    case "single_choice":
    case "select_all":
      return <ChoiceQuestionEditor {...shared} draft={draft} onChange={onChange} />;
    case "true_false":
      return <TrueFalseQuestionEditor {...shared} draft={draft} onChange={onChange} />;
    case "matching":
      return <MatchingQuestionEditor {...shared} draft={draft} onChange={onChange} />;
    case "grouping":
      return <GroupingQuestionEditor {...shared} draft={draft} onChange={onChange} />;
    case "ordering":
      return <OrderingQuestionEditor {...shared} draft={draft} onChange={onChange} />;
  }
}
