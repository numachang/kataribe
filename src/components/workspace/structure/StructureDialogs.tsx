import type { StructureRequest } from "../../../features/structure/structureRequest";
import { AddCharacterDialog } from "./AddCharacterDialog";
import { AddSceneDialog } from "./AddSceneDialog";
import { AddWorldDocumentDialog } from "./AddWorldDocumentDialog";
import { RemoveConfirmDialog } from "./RemoveConfirmDialog";

interface StructureDialogsProps {
  /** 開いているダイアログ。無ければ何も出さない。 */
  request: StructureRequest | null;
  onClose: () => void;
}

/** 目次から求められた構成の操作に合うダイアログを出す。 */
export function StructureDialogs({ request, onClose }: StructureDialogsProps) {
  if (request === null) {
    return null;
  }
  switch (request.kind) {
    case "add_character":
      return <AddCharacterDialog onClose={onClose} />;
    case "add_world_document":
      return <AddWorldDocumentDialog onClose={onClose} />;
    case "add_scene":
      return <AddSceneDialog chapter={request.chapter} before={request.before} onClose={onClose} />;
    case "remove":
      return <RemoveConfirmDialog edit={request.edit} onClose={onClose} />;
  }
}
