import { useEffect, useState } from "react";
import { useBackend } from "../../api/context";
import type { GenrePreset, NewProject, ProjectOverview, Rating } from "../../api/types";
import { toErrorMessage } from "../../lib/errorMessage";
import { useUiStore } from "../../store/uiStore";
import { Dialog } from "../Dialog";
import "./NewProjectDialog.css";

const DEFAULT_TARGET_LENGTH = 30000;

const RATING_LABELS: Record<Rating, string> = {
  general: "全年齢",
  r15: "R15",
  r18: "R18",
};

interface NewProjectDialogProps {
  onClose: () => void;
  onCreated: (overview: ProjectOverview) => void;
}

/** 「新しい作品」ダイアログ。作品情報を入力し、保存先フォルダを選んでから作成する。 */
export function NewProjectDialog({ onClose, onCreated }: NewProjectDialogProps) {
  const backend = useBackend();
  const showToast = useUiStore((state) => state.showToast);

  const [genres, setGenres] = useState<GenrePreset[]>([]);
  const [title, setTitle] = useState("");
  const [author, setAuthor] = useState("");
  const [genre, setGenre] = useState("");
  const [genreNote, setGenreNote] = useState("");
  const [rating, setRating] = useState<Rating>("general");
  const [targetLength, setTargetLength] = useState(DEFAULT_TARGET_LENGTH);
  const [idea, setIdea] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void backend.listGenres().then((list) => {
      if (cancelled) {
        return;
      }
      setGenres(list);
      setGenre((current) => current || (list[0]?.id ?? ""));
    });
    return () => {
      cancelled = true;
    };
  }, [backend]);

  async function handlePickFolder(): Promise<void> {
    const picked = await backend.pickFolder();
    if (picked) {
      setFolder(picked);
    }
  }

  const canCreate = title.trim().length > 0 && folder !== null && !isCreating;

  async function handleCreate(): Promise<void> {
    if (!canCreate || folder === null) {
      return;
    }
    setIsCreating(true);
    const project: NewProject = {
      title: title.trim(),
      author: author.trim().length > 0 ? author.trim() : null,
      genre,
      genre_note: genreNote.trim().length > 0 ? genreNote.trim() : null,
      rating,
      target_length: targetLength,
      idea,
    };
    try {
      const overview = await backend.createProject(folder, project);
      onCreated(overview);
    } catch (error) {
      showToast(toErrorMessage(error, "作品を作成できませんでした。"), "error");
    } finally {
      setIsCreating(false);
    }
  }

  return (
    <Dialog title="新しい作品" onClose={onClose} wide>
      <div className="new-project-dialog">
        <label className="app-field">
          <span>題名（必須）</span>
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="作品の題名"
          />
        </label>

        <label className="app-field">
          <span>作者（任意）</span>
          <input value={author} onChange={(event) => setAuthor(event.target.value)} />
        </label>

        <div className="new-project-dialog__row">
          <label className="app-field">
            <span>ジャンル</span>
            <select value={genre} onChange={(event) => setGenre(event.target.value)}>
              {genres.map((preset) => (
                <option key={preset.id} value={preset.id}>
                  {preset.label}
                </option>
              ))}
            </select>
          </label>

          <label className="app-field">
            <span>年齢区分</span>
            <select value={rating} onChange={(event) => setRating(event.target.value as Rating)}>
              {(Object.keys(RATING_LABELS) as Rating[]).map((value) => (
                <option key={value} value={value}>
                  {RATING_LABELS[value]}
                </option>
              ))}
            </select>
          </label>
        </div>

        <label className="app-field">
          <span>ジャンルの補足（任意）</span>
          <input
            value={genreNote}
            onChange={(event) => setGenreNote(event.target.value)}
            placeholder="例: 館もの。本格"
          />
        </label>

        <label className="app-field">
          <span>目標文字数</span>
          <input
            type="number"
            min={1000}
            step={1000}
            value={targetLength}
            onChange={(event) =>
              setTargetLength(Number(event.target.value) || DEFAULT_TARGET_LENGTH)
            }
          />
        </label>

        <label className="app-field">
          <span>企画の種</span>
          <textarea
            rows={4}
            value={idea}
            onChange={(event) => setIdea(event.target.value)}
            placeholder="どんな物語を書きたいか、数行で自由に書いてください。"
          />
        </label>

        <div className="new-project-dialog__folder">
          <span className="new-project-dialog__folder-label">保存先フォルダ</span>
          <p className="new-project-dialog__hint">
            選んだフォルダそのものを作品フォルダにします。空のフォルダを選んでください。
          </p>
          <div className="new-project-dialog__folder-row">
            <button type="button" className="app-button" onClick={() => void handlePickFolder()}>
              フォルダを選ぶ
            </button>
            <span className="new-project-dialog__folder-path">{folder ?? "未選択"}</span>
          </div>
        </div>

        <div className="new-project-dialog__actions">
          <button type="button" className="app-button" onClick={onClose}>
            キャンセル
          </button>
          <button
            type="button"
            className="app-button app-button--primary"
            disabled={!canCreate}
            onClick={() => void handleCreate()}
          >
            {isCreating ? "作成しています…" : "作成する"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
