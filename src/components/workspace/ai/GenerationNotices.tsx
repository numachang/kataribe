import type { GenerationStepDisplay } from "../../../features/generation/eventAccumulator";
import "./GenerationNotices.css";

interface GenerationNoticesProps {
  steps: GenerationStepDisplay[];
}

/**
 * 生成中に届いた注意書き（古くなった文書の知らせ、作り直した理由など）を、回をまたいでまとめて見せる。
 * 変更案の確認に切り替わると、回ごとの表示（GenerationProgress）は消えるので、確認する前に読めるようにここで見せる。
 */
export function GenerationNotices({ steps }: GenerationNoticesProps) {
  const notices = steps.flatMap((step) => step.notices);
  if (notices.length === 0) {
    return null;
  }
  return (
    <ul className="generation-notices" aria-label="生成中の注意書き">
      {notices.map((notice, index) => (
        <li
          // biome-ignore lint/suspicious/noArrayIndexKey: 注意書きには一意な id がないため
          key={index}
          className={`generation-notices__item generation-notices__item--${notice.level}`}
        >
          {notice.message}
        </li>
      ))}
    </ul>
  );
}
