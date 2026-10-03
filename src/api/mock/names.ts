// 人物の名前の突き合わせ。kataribe-engine の `names.rs` と同じ規則。
// シーンの視点人物・登場人物は名前の文字列で書かれるので、その文字列がどの人物を指すかを、
// 人物を消すときの参照の確認が同じ規則で判断する。

function withoutWhitespace(name: string): string {
  return name.replace(/\s/g, "");
}

/** 空白の有無を無視して名前が一致するか（「霧島 凛」と「霧島凛」を同一人物とみなす）。 */
function isSamePerson(left: string, right: string): boolean {
  return withoutWhitespace(left) === withoutWhitespace(right);
}

/** `name`（姓や名のどれか）が、`characterName` を空白で区切った一部と一致するか。「凛」だけの書き方を拾う。 */
function isPartOfName(characterName: string, name: string): boolean {
  const trimmed = name.trim();
  return trimmed !== "" && characterName.split(/\s+/).includes(trimmed);
}

/**
 * シーンに書かれた `name` が、`characterName` の人物を指しているか。
 * 空白の違いと、「凛」のように姓や名だけの書き方を同一人物とみなす。空の名前はだれも指さない。
 */
export function refersTo(characterName: string, name: string): boolean {
  return (
    name.trim() !== "" && (isSamePerson(characterName, name) || isPartOfName(characterName, name))
  );
}
