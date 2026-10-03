import { useMemo, useState } from "react";
import type { ProjectOverview } from "../../api/types";
import { additionalWorldDocumentNameOfPath } from "../../lib/entryPaths";
import { slugProblem } from "../../lib/slug";
import { useWorkspaceStore } from "../../store/workspaceStore";

/** 世界観の概要（`world/overview.md`）の名前。足す資料には使えない（slug の規則には合うので別に断る）。 */
const OVERVIEW_NAME = "overview";

/** 目次にある、足した世界観の資料の name（小文字にそろえる。Windows は大文字小文字を区別しない）。 */
function usedWorldDocumentNames(overview: ProjectOverview | null): Set<string> {
  const names = new Set<string>();
  for (const section of overview?.sections ?? []) {
    if (section.kind !== "world") {
      continue;
    }
    for (const entry of section.entries) {
      const name = entry.path === null ? null : additionalWorldDocumentNameOfPath(entry.path);
      if (name !== null) {
        names.add(name.toLowerCase());
      }
    }
  }
  return names;
}

function worldDocumentNameProblem(name: string, usedNames: Set<string>): string | null {
  const ruleProblem = slugProblem(name);
  if (ruleProblem !== null) {
    return ruleProblem;
  }
  if (name === OVERVIEW_NAME) {
    return "「overview」は世界観の概要の名前なので使えません。";
  }
  if (usedNames.has(name)) {
    return "すでにある資料の名前です。";
  }
  return null;
}

/**
 * 世界観の資料を足すダイアログの「ファイル名」の欄（自分で書くときも AI に作らせるときも同じ）。
 *
 * 書いた名前は、送る前に規則（小文字の英数字とハイフン・概要の名前でない）と、目次にある資料の名前との重なりを確かめ、
 * 使えなければ理由を返す（生成を始めてからでは、ダイアログが閉じて書いた指示を失うため）。
 * 目次が古いなどで見逃した分は、本物の確認（`check_name`）が最後に断る。
 * 空欄は「題から自動で決める」なので、確かめず null を送る。
 */
export function useWorldDocumentNameField() {
  const overview = useWorkspaceStore((state) => state.overview);
  const [nameText, setNameText] = useState("");
  const usedNames = useMemo(() => usedWorldDocumentNames(overview), [overview]);

  const nameToSend = nameText.trim() === "" ? null : nameText.trim();

  return {
    nameText,
    changeName: setNameText,
    /** 追加・生成に渡すファイル名。空欄（空白だけを含む）なら null。 */
    nameToSend,
    /** 書いた名前が使えない理由。使える、または空欄（自動）なら null。 */
    problem: nameToSend === null ? null : worldDocumentNameProblem(nameToSend, usedNames),
  };
}

export type WorldDocumentNameField = ReturnType<typeof useWorldDocumentNameField>;
