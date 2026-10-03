import type { RenumberedChapter, SceneReference, StructurePlan } from "../../../api/types";
import { manuscriptAmong, trashedFilesOf } from "../../../lib/trashSummary";
import { TrashedFileList } from "./TrashedFileList";
import "./StructureDialog.css";

const TRASH_FOLDER_NOTE = "ゴミ箱は作品フォルダの .kataribe/trash/ です。";

function describeRole(reference: SceneReference): string {
  if (reference.as_pov && reference.as_character) {
    return "視点・登場人物";
  }
  return reference.as_pov ? "視点" : "登場人物";
}

function describeRenumbering(chapter: RenumberedChapter): string {
  const before =
    chapter.title === null
      ? `第${Number(chapter.from)}章`
      : `第${Number(chapter.from)}章「${chapter.title}」`;
  return `${before} → 第${Number(chapter.to)}章`;
}

interface RemovalDetailsProps {
  plan: StructurePlan;
}

/** 削除の確認で見せる材料。何がゴミ箱へ移るか、番号が変わる章はどれか、消すことで参照が切れるシーンはどれか、注意書き。 */
export function RemovalDetails({ plan }: RemovalDetailsProps) {
  const trashedFiles = trashedFilesOf(plan.change_set);
  const manuscript = manuscriptAmong(trashedFiles);

  return (
    <>
      <p className="structure-dialog__summary">{plan.change_set.summary}</p>

      <section className="structure-dialog__section">
        <h3>ゴミ箱へ移すもの</h3>
        <TrashedFileList files={trashedFiles} />
        {manuscript.count > 0 && (
          <p className="structure-dialog__emphasis">
            本文 {manuscript.count} ファイル（計 {manuscript.chars.toLocaleString("ja-JP")}{" "}
            字）もゴミ箱へ移ります。
          </p>
        )}
      </section>

      {plan.renumbered.length > 0 && (
        <section className="structure-dialog__section">
          <h3>番号が変わる章</h3>
          <ul className="structure-dialog__list">
            {plan.renumbered.map((chapter) => (
              <li key={chapter.from}>{describeRenumbering(chapter)}</li>
            ))}
          </ul>
          <p className="structure-dialog__note">
            章立てのファイルと本文のフォルダの名前も、新しい番号に変わります。
          </p>
        </section>
      )}

      {plan.references.length > 0 && (
        <section className="structure-dialog__section">
          <h3>この人物を挙げているシーン</h3>
          <ul className="structure-dialog__list">
            {plan.references.map((reference) => (
              <li key={`${reference.chapter}/${reference.scene}`}>
                第{Number(reference.chapter)}章「{reference.chapter_title}」 {reference.scene_title}
                （{describeRole(reference)}）
              </li>
            ))}
          </ul>
          <p className="structure-dialog__note">
            シーンに書かれた名前は書き換えません。消したあとで、必要に応じて直してください。
          </p>
        </section>
      )}

      {plan.notices.length > 0 && (
        <ul className="structure-dialog__list structure-dialog__notices">
          {plan.notices.map((notice) => (
            <li key={notice}>{notice}</li>
          ))}
        </ul>
      )}

      <p className="structure-dialog__note">{TRASH_FOLDER_NOTE}</p>
    </>
  );
}
