import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import { slugProblem } from "../../lib/slug";

const SUGGESTION_DELAY_MS = 250;

/**
 * 人物を足すダイアログの「ID」の欄。
 *
 * 利用者が触るまでは、読み・名前の入力に合わせて提案（`suggestCharacterId`）に追従する。
 * 入力のたびに問い合わせると無駄が多いので、手が止まって少し待ってから問い合わせ、
 * 返ってきたときにはもう古い入力への答えなら捨てる。
 * 利用者が触ったら追従を止め、書いた ID をそのまま使う。触っていなければ ID は送らず、Rust に決めさせる
 * （画面に出ている提案と、実際に使われる ID がずれないよう、同じ関数で決まる）。
 * 利用者が書いた ID は、送る前に規則（小文字の英数字とハイフン）に合うか確かめ、合わなければ理由を返す
 * （本物に送ってから失敗するより、書いている最中に知らせるため）。提案は規則に合うものだけが返るので確かめない。
 */
export function useCharacterIdField(reading: string, name: string) {
  const backend = useBackend();
  const [idText, setIdText] = useState("");
  const [isEdited, setEdited] = useState(false);

  useEffect(() => {
    if (isEdited) {
      return;
    }
    if (reading.trim() === "" && name.trim() === "") {
      setIdText("");
      return;
    }
    let isStale = false;
    const timer = setTimeout(() => {
      backend
        .suggestCharacterId(reading, name)
        .then((suggestion) => {
          if (!isStale) {
            setIdText(suggestion);
          }
        })
        .catch(() => {
          // 提案が出せなくても追加はできる（ID を送らなければ、追加のときに Rust が決める）
        });
    }, SUGGESTION_DELAY_MS);
    return () => {
      isStale = true;
      clearTimeout(timer);
    };
  }, [backend, reading, name, isEdited]);

  const idToSend = isEdited && idText.trim() !== "" ? idText.trim() : null;

  return {
    idText,
    /** 利用者が欄に入力したとき。空にしたら、また提案に追従する。 */
    changeId(text: string) {
      setIdText(text);
      setEdited(text !== "");
    },
    /** 追加の操作に渡す ID。触っていない（または空）なら null。 */
    idToSend,
    /** 書いた ID が使えない理由。使える、または送らない（自動）なら null。 */
    problem: idToSend !== null ? slugProblem(idToSend) : null,
  };
}
