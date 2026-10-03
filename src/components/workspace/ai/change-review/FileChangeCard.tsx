import type { FileChange } from "../../../../api/types";
import { TrashCard } from "./TrashCard";
import { WriteCard } from "./WriteCard";

interface FileChangeCardProps {
  file: FileChange;
}

/** 変更案の 1 つの変更。書き込みは内容を、ゴミ箱へ移すものは何が移るかを見せる。 */
export function FileChangeCard({ file }: FileChangeCardProps) {
  switch (file.kind) {
    case "write":
      return <WriteCard file={file} />;
    case "trash":
      return <TrashCard file={file} />;
  }
}
