#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

function replaceOnce(path, before, after) {
  const source = readFileSync(path, "utf8");
  const count = source.split(before).length - 1;
  if (count !== 1) throw new Error(`${path}: expected 1 match, found ${count}`);
  writeFileSync(path, source.replace(before, after));
}

replaceOnce(
  "src-tauri/src/document_more.rs",
  `    #[test]\n    fn append_payload_uses_format_neutral_source_tag_input() {\n        let payload = build_append_payload(\n            "Physiology",\n            "pdf",\n            vec![serde_json::json!({"type":"basic","front":"Q","back":"A","tags":[]})],\n        )\n        .unwrap();\n        assert!(payload.contains("\\\"PDF\\\""));\n    }`,
  `    #[test]\n    fn append_payload_keeps_only_card_subject_tags() {\n        let payload = build_append_payload(\n            "Physiology",\n            vec![serde_json::json!({\n                "type": "basic",\n                "front": "Q",\n                "back": "A",\n                "tags": ["Cardiology"]\n            })],\n        )\n        .unwrap();\n        let value: Value = serde_json::from_str(&payload).unwrap();\n        assert_eq!(value["defaultTags"], serde_json::json!([]));\n        assert_eq!(value["cards"][0]["tags"], serde_json::json!(["Cardiology"]));\n        assert!(!payload.contains("AI generated"));\n        assert!(!payload.contains("\\\"PDF\\\""));\n    }`,
);

replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  `            <th>Local period</th>\n            <th>Cards</th>\n            <th>Reviews</th>\n            <th>New</th>\n            <th>Remembered</th>\n            <th>Confidence</th>\n            <th>Study time</th>`,
  `            <th>Local period</th>\n            <th>Cards</th>\n            <th>Reviews</th>\n            <th>Wrong</th>\n            <th>Hard</th>\n            <th>Medium</th>\n            <th>Easy</th>\n            <th>Remembered</th>\n            <th>Study time</th>`,
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  `              <td>{bucket.cardsPracticed}</td>\n              <td>{bucket.reviewCount}</td>\n              <td>{bucket.newCards}</td>\n              <td>{formatPercent(bucket.correctRate)}</td>\n              <td>{formatConfidence(bucket.averageConfidence)}</td>\n              <td>{formatStudyTime(bucket.studyTimeMs)}</td>`,
  `              <td>{bucket.cardsPracticed}</td>\n              <td>{bucket.reviewCount}</td>\n              <td>{bucket.wrongCount}</td>\n              <td>{bucket.hardCount}</td>\n              <td>{bucket.mediumCount}</td>\n              <td>{bucket.easyCount}</td>\n              <td>{formatPercent(bucket.correctRate)}</td>\n              <td>{formatStudyTime(bucket.studyTimeMs)}</td>`,
);

replaceOnce(
  "src/styles/features/ui-overhaul.css",
  `.practice-flip-front {\n  transform: rotateY(0deg) translateZ(1px);\n  -webkit-transform: rotateY(0deg) translateZ(1px);\n}\n\n.practice-flip-back {\n  transform: rotateY(180deg) translateZ(1px);\n  -webkit-transform: rotateY(180deg) translateZ(1px);\n}`,
  `.practice-flip-front {\n  z-index: 2;\n  transform: rotateY(0deg) translateZ(1px);\n  -webkit-transform: rotateY(0deg) translateZ(1px);\n}\n\n.practice-flip-back {\n  z-index: 1;\n  transform: rotateY(180deg) translateZ(1px);\n  -webkit-transform: rotateY(180deg) translateZ(1px);\n}\n\n.practice-flip-card[data-flipped="true"] .practice-flip-front {\n  z-index: 1;\n}\n\n.practice-flip-card[data-flipped="true"] .practice-flip-back {\n  z-index: 2;\n}`,
);

console.log("Finished PR #41 follow-up regression fixes.");
