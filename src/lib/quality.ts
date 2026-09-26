import type { QualityIssue, QualityMetrics, QualityReport } from "../api/types";
import { countGraphemes } from "./graphemes";
import { computeTextStats } from "./textStats";

// kataribe-text クレートの `quality::analyze` を Rust 実装が来るまでの間、
// 画面の開発・テストに使えるだけの精度で再現したもの。しきい値は経験的な目安。

const SENTENCE_SPLIT = /(?<=[。！？])/u;
const HAN_CHAR = /\p{Script=Han}/u;
const HANGUL_CHAR = /\p{Script=Hangul}/u;
const LONG_LATIN_RUN = /[A-Za-z]{8,}/u;
const MARKDOWN_ARTIFACT = /(^#{1,6}\s|\*\*[^*]+\*\*|^```|^-\s\[[ x]\])/mu;
const META_COMMENTARY_PHRASES = [
  "以下は本文です",
  "以下に本文を",
  "承知しました",
  "かしこまりました",
  "ご要望どおり",
  "AIとして",
  "as an ai",
];

const REPEATED_NGRAM_SIZE = 8;
const REPEATED_NGRAM_RATIO_WARNING = 0.15;
const MONOTONOUS_ENDING_RUN_WARNING = 4;
const SHORT_RATIO = 0.7;
const LONG_RATIO = 1.5;

function splitSentences(text: string): string[] {
  return text
    .split(SENTENCE_SPLIT)
    .map((sentence) => sentence.trim())
    .filter((sentence) => sentence.length > 0);
}

function computeDialogueRatio(text: string): number {
  const lines = text.split(/\r\n|\r|\n/).filter((line) => line.trim().length > 0);
  if (lines.length === 0) {
    return 0;
  }
  const dialogueLines = lines.filter((line) => /^[「『]/.test(line.trim())).length;
  return dialogueLines / lines.length;
}

function computeKanjiRatio(text: string): number {
  const withoutWhitespace = Array.from(text.replace(/\s+/gu, ""));
  if (withoutWhitespace.length === 0) {
    return 0;
  }
  const kanjiCount = withoutWhitespace.filter((char) => HAN_CHAR.test(char)).length;
  return kanjiCount / withoutWhitespace.length;
}

function computeAverageSentenceLength(sentences: string[]): number {
  if (sentences.length === 0) {
    return 0;
  }
  const total = sentences.reduce((sum, sentence) => sum + countGraphemes(sentence), 0);
  return Math.round((total / sentences.length) * 10) / 10;
}

/** 文末の終わり方（最後の非約物文字 1 文字）を取り出す。 */
function sentenceEnding(sentence: string): string {
  const withoutTrailingPunctuation = sentence.replace(/[。！？」』）]+$/u, "");
  return withoutTrailingPunctuation.at(-1) ?? "";
}

function computeLongestSameEndingRun(sentences: string[]): number {
  let longest = 0;
  let current = 0;
  let previousEnding: string | null = null;
  for (const sentence of sentences) {
    const ending = sentenceEnding(sentence);
    if (ending !== "" && ending === previousEnding) {
      current += 1;
    } else {
      current = 1;
    }
    longest = Math.max(longest, current);
    previousEnding = ending;
  }
  return longest;
}

function computeRepeatedPhraseRatio(text: string): number {
  const graphemes = Array.from(text.replace(/\s+/gu, ""));
  if (graphemes.length < REPEATED_NGRAM_SIZE * 2) {
    return 0;
  }
  const seen = new Map<string, number>();
  let total = 0;
  for (let index = 0; index <= graphemes.length - REPEATED_NGRAM_SIZE; index += 1) {
    const gram = graphemes.slice(index, index + REPEATED_NGRAM_SIZE).join("");
    seen.set(gram, (seen.get(gram) ?? 0) + 1);
    total += 1;
  }
  const repeated = [...seen.values()].filter((count) => count > 1).length;
  return total === 0 ? 0 : Math.round((repeated / total) * 1000) / 1000;
}

function findFirstMatchLine(text: string, predicate: (line: string) => boolean): string | null {
  const lines = text.split(/\r\n|\r|\n/);
  return lines.find(predicate) ?? null;
}

function detectMetaCommentary(text: string): QualityIssue | null {
  const lower = text.toLowerCase();
  const phrase = META_COMMENTARY_PHRASES.find((candidate) =>
    lower.includes(candidate.toLowerCase()),
  );
  if (!phrase) {
    return null;
  }
  const excerpt = findFirstMatchLine(text, (line) =>
    line.toLowerCase().includes(phrase.toLowerCase()),
  );
  return {
    kind: "meta_commentary",
    severity: "error",
    message: "本文に地の文らしくない前置き・応答が混入しています。",
    excerpt,
  };
}

function detectMarkdownArtifact(text: string): QualityIssue | null {
  if (!MARKDOWN_ARTIFACT.test(text)) {
    return null;
  }
  const excerpt = findFirstMatchLine(text, (line) => MARKDOWN_ARTIFACT.test(line));
  return {
    kind: "markdown_artifact",
    severity: "warning",
    message: "Markdown の記法（見出し・強調など）が本文に混入しています。",
    excerpt,
  };
}

function detectForeignScript(text: string): QualityIssue | null {
  const hangulLine = findFirstMatchLine(text, (line) => HANGUL_CHAR.test(line));
  if (hangulLine) {
    return {
      kind: "foreign_script",
      severity: "warning",
      message: "ハングルと思われる文字が混入しています。",
      excerpt: hangulLine,
    };
  }
  const latinLine = findFirstMatchLine(text, (line) => LONG_LATIN_RUN.test(line));
  if (latinLine) {
    return {
      kind: "foreign_script",
      severity: "info",
      message: "長い英単語の連なりが含まれています。意図した表記か確認してください。",
      excerpt: latinLine,
    };
  }
  return null;
}

function detectRepeatedSentence(sentences: string[]): QualityIssue | null {
  const seen = new Set<string>();
  for (const sentence of sentences) {
    if (sentence.length < 6) {
      continue;
    }
    if (seen.has(sentence)) {
      return {
        kind: "repeated_sentence",
        severity: "warning",
        message: "同じ文が繰り返し使われています。",
        excerpt: sentence,
      };
    }
    seen.add(sentence);
  }
  return null;
}

function detectRepeatedPhrase(ratio: number): QualityIssue | null {
  if (ratio < REPEATED_NGRAM_RATIO_WARNING) {
    return null;
  }
  return {
    kind: "repeated_phrase",
    severity: "warning",
    message: "同じ言い回しが多用されています。",
    excerpt: null,
  };
}

function detectMonotonousEndings(run: number, sentences: string[]): QualityIssue | null {
  if (run < MONOTONOUS_ENDING_RUN_WARNING) {
    return null;
  }
  return {
    kind: "monotonous_endings",
    severity: "warning",
    message: `文末が ${run} 文連続で同じ終わり方になっています。`,
    excerpt: sentences.at(-1) ?? null,
  };
}

function detectLengthIssue(chars: number, targetChars: number | null): QualityIssue | null {
  if (targetChars === null || targetChars <= 0) {
    return null;
  }
  if (chars < targetChars * SHORT_RATIO) {
    return {
      kind: "too_short",
      severity: "warning",
      message: `目標文字数 ${targetChars} に対して分量が不足しています（${chars} 字）。`,
      excerpt: null,
    };
  }
  if (chars > targetChars * LONG_RATIO) {
    return {
      kind: "too_long",
      severity: "warning",
      message: `目標文字数 ${targetChars} に対して分量が超過しています（${chars} 字）。`,
      excerpt: null,
    };
  }
  return null;
}

function detectUnbalancedBrackets(text: string): QualityIssue | null {
  const opens = (text.match(/「/gu) ?? []).length;
  const closes = (text.match(/」/gu) ?? []).length;
  if (opens === closes) {
    return null;
  }
  return {
    kind: "unbalanced_brackets",
    severity: "error",
    message: `「」の数が合っていません（「 が ${opens} 個、」 が ${closes} 個）。`,
    excerpt: null,
  };
}

/** 本文を機械的に検査し、指標と問題点を返す。 */
export function analyzeQualityText(text: string, targetChars: number | null): QualityReport {
  const stats = computeTextStats(text);
  const sentences = splitSentences(text);
  const repeatedPhraseRatio = computeRepeatedPhraseRatio(text);
  const longestSameEndingRun = computeLongestSameEndingRun(sentences);

  const metrics: QualityMetrics = {
    dialogue_ratio: Math.round(computeDialogueRatio(text) * 1000) / 1000,
    kanji_ratio: Math.round(computeKanjiRatio(text) * 1000) / 1000,
    average_sentence_length: computeAverageSentenceLength(sentences),
    longest_same_ending_run: longestSameEndingRun,
    repeated_phrase_ratio: repeatedPhraseRatio,
  };

  const issues = [
    detectMetaCommentary(text),
    detectMarkdownArtifact(text),
    detectForeignScript(text),
    detectRepeatedSentence(sentences),
    detectRepeatedPhrase(repeatedPhraseRatio),
    detectMonotonousEndings(longestSameEndingRun, sentences),
    detectLengthIssue(stats.chars, targetChars),
    detectUnbalancedBrackets(text),
  ].filter((issue): issue is QualityIssue => issue !== null);

  return { stats, metrics, issues };
}
