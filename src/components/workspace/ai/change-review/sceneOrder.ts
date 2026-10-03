interface Identified {
  id: string;
}

/** 前から順に増えていく部分列のうち、いちばん長いものの添字。同じ長さなら先に見つかったもの。 */
function longestIncreasingIndices(values: number[]): Set<number> {
  const chainsEndingAt: number[][] = [];
  values.forEach((value, index) => {
    const bestEarlierChain = chainsEndingAt
      .filter((_, earlier) => (values[earlier] ?? Number.POSITIVE_INFINITY) < value)
      .reduce<number[]>((longest, chain) => (chain.length > longest.length ? chain : longest), []);
    chainsEndingAt.push([...bestEarlierChain, index]);
  });
  const longest = chainsEndingAt.reduce<number[]>(
    (best, chain) => (chain.length > best.length ? chain : best),
    [],
  );
  return new Set(longest);
}

/**
 * 両方にあるシーンのうち、相手と並びが入れ替わったものの id。
 *
 * 並びを保ったまま残るいちばん長い列に入らないシーンを「動いた」とみなす。
 * 1 つを先頭から末尾へ移したときに、間のシーンまで動いたことにしないため。
 */
export function movedSceneIds(scenes: Identified[], counterpartScenes: Identified[]): Set<string> {
  const counterpartIds = new Set(counterpartScenes.map((scene) => scene.id));
  const commonIds = scenes.map((scene) => scene.id).filter((id) => counterpartIds.has(id));
  const counterpartOrder = counterpartScenes
    .map((scene) => scene.id)
    .filter((id) => commonIds.includes(id));
  const positions = commonIds.map((id) => counterpartOrder.indexOf(id));
  const unmoved = longestIncreasingIndices(positions);
  return new Set(commonIds.filter((_, index) => !unmoved.has(index)));
}
