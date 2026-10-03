import type { FileChange } from "../../../../api/types";
import { MoveCard } from "./MoveCard";
import { TrashCard } from "./TrashCard";
import { WriteCard } from "./WriteCard";

interface FileChangeCardProps {
  file: FileChange;
}

/**
 * 変更案の 1 つの変更。書き込みは内容を、ゴミ箱へ移すものは何が移るかを、移動は移動元と移動先を見せる。
 * 状態を確かめるだけの変更（expect）は何も変えないので、見せるものが無い。
 */
export function FileChangeCard({ file }: FileChangeCardProps) {
  switch (file.kind) {
    case "write":
      return <WriteCard file={file} />;
    case "trash":
      return <TrashCard file={file} />;
    case "move":
      return <MoveCard file={file} />;
    case "expect":
      return null;
  }
}
