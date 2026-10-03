import type { TrashedFile } from "../../../api/types";
import "./TrashedFileList.css";

interface TrashedFileListProps {
  files: TrashedFile[];
}

function describeFile(file: TrashedFile): string {
  if (file.base_hash === null) {
    return "テキストとして読めないため、移せません";
  }
  return `${file.chars.toLocaleString("ja-JP")} 字`;
}

/** ゴミ箱へ移すファイルの一覧（パスと文字数）。削除の確認と、変更案の表示が共有する。 */
export function TrashedFileList({ files }: TrashedFileListProps) {
  return (
    <ul className="trashed-files">
      {files.map((file) => (
        <li key={file.path} className="trashed-files__item">
          <span className="trashed-files__path">{file.path}</span>
          <span className="trashed-files__chars">{describeFile(file)}</span>
        </li>
      ))}
    </ul>
  );
}
